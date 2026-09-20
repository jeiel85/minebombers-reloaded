import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  BGM_FILE,
  GAME_FILES,
  NOTICE_FILES,
  SFX_FILES,
  MissingGameDataError,
  decodeVoc,
  loadOriginalFiles,
} from '../src/assets.js';

// Nothing here needs the original game files. The few tests that check the real thing run only when
// the developer has a copy (MB_GAME_DIR, or res/minebomb) and are skipped otherwise.
const realDir = process.env.MB_GAME_DIR || fileURLToPath(new URL('../../res/minebomb/', import.meta.url));
const hasRealFiles = existsSync(join(realDir, 'TITLEBE.SPY'));
const needsGameFiles = { skip: !hasRealFiles && 'needs your own Mine Bombers 3.11 files (set MB_GAME_DIR)' };
const original = (name) => new Uint8Array(readFileSync(join(realDir, name)));

// Quoted names inside `const <name>: [&str; N] = [ ... ];` in the Rust crate
function rustList(source, name) {
  const block = source.match(new RegExp(`const ${name}: \\[&str; \\d+\\] = \\[([^\\]]*)\\]`));
  assert.ok(block, `${name} not found in crates/mb-wasm/src/lib.rs`);
  return [...block[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

test('GAME_FILES lists exactly what the wasm module asks the host for', () => {
  const rust = readFileSync(new URL('../../crates/mb-wasm/src/lib.rs', import.meta.url), 'utf8');
  const expected = [...rustList(rust, 'SCREEN_ASSET_FILES'), ...rustList(rust, 'CLASSIC_MAP_FILES')];
  assert.deepEqual(GAME_FILES, expected);
});

test('file lists have no duplicates', () => {
  const all = [...GAME_FILES, ...SFX_FILES, BGM_FILE, ...NOTICE_FILES];
  assert.equal(new Set(all).size, all.length);
});

test('decodeVoc treats a file without the block prefix as raw 8-bit PCM at 11025 Hz', () => {
  const bytes = Uint8Array.from([0x80, 0x80, 0x80, 0x80, 0x7f, 0x80, 0x7f, 0x7f]);
  const { pcm, sampleRate } = decodeVoc(bytes);
  assert.equal(sampleRate, 11025);
  assert.deepEqual([...pcm], [...bytes]);
});

test('decodeVoc reads the block layout (4-byte prefix, type 1 block, time constant)', () => {
  // prefix | type 1, 24-bit size (samples + 2), time constant 0xa6, codec 0 | samples
  const samples = [10, 20, 30, 40, 50];
  const bytes = Uint8Array.from([0xe8, 0x32, 0x00, 0x00, 0x01, samples.length + 2, 0x00, 0x00, 0xa6, 0x00, ...samples]);
  const { pcm, sampleRate } = decodeVoc(bytes);
  assert.equal(sampleRate, Math.round(1000000 / (256 - 0xa6))); // 11111 Hz
  assert.deepEqual([...pcm], samples);
});

test('decodeVoc reads a 24-bit block size', () => {
  const samples = new Array(70000).fill(0x90);
  const size = samples.length + 2;
  const bytes = new Uint8Array(10 + samples.length);
  bytes.set([0, 0, 0, 0, 0x01, size & 0xff, (size >> 8) & 0xff, (size >> 16) & 0xff, 0xa6, 0x00]);
  bytes.fill(0x90, 10);
  assert.equal(decodeVoc(bytes).pcm.length, 70000);
});

test('loadOriginalFiles returns the fetched bytes', async () => {
  const fetchFn = async (url) => ({ ok: true, arrayBuffer: async () => new Uint8Array([url.length, 7]).buffer });
  const files = await loadOriginalFiles(['A.SPY', 'B.MNE'], './data/', fetchFn);
  assert.deepEqual([...files.keys()].sort(), ['A.SPY', 'B.MNE']);
  assert.deepEqual([...files.get('A.SPY')], ['./data/A.SPY'.length, 7]);
});

test('loadOriginalFiles names every file it could not get, in request order', async () => {
  const fetchFn = async (url) => {
    if (url.endsWith('C.MNE')) throw new TypeError('network down');
    if (url.endsWith('A.SPY') || url.endsWith('D.VOC')) return { ok: false, status: 404 };
    return { ok: true, arrayBuffer: async () => new ArrayBuffer(1) };
  };
  await assert.rejects(
    loadOriginalFiles(['A.SPY', 'B.MNE', 'C.MNE', 'D.VOC'], './data/', fetchFn),
    (err) => {
      assert.ok(err instanceof MissingGameDataError);
      assert.deepEqual(err.missing, ['A.SPY', 'C.MNE', 'D.VOC']);
      assert.match(err.message, /stage_web_data\.mjs/);
      return true;
    }
  );
});

// ---- checks against the real files, for developers who have them ----

test('every file the page uses exists in a real Mine Bombers 3.11 copy', needsGameFiles, () => {
  for (const name of [...GAME_FILES, ...SFX_FILES, BGM_FILE]) {
    assert.ok(existsSync(join(realDir, name)), `${name} is missing from ${realDir}`);
  }
});

test('every real sound effect decodes to audible PCM at a rate Web Audio accepts', needsGameFiles, () => {
  for (const name of SFX_FILES) {
    const { pcm, sampleRate } = decodeVoc(original(name));
    assert.ok(pcm.length > 100, `${name}: ${pcm.length} samples`);
    assert.ok(sampleRate >= 8000 && sampleRate <= 96000, `${name}: ${sampleRate} Hz`);
  }
});

test('the real background music is a Scream Tracker 3 module', needsGameFiles, () => {
  const bytes = original(BGM_FILE);
  assert.equal(String.fromCharCode(...bytes.subarray(0x2c, 0x30)), 'SCRM');
});
