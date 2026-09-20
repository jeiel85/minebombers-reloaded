import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import {
  BGM_FILE,
  GAME_FILES,
  NOTICE_FILES,
  SFX_FILES,
  MissingGameDataError,
  decodeVoc,
  loadOriginalFiles,
} from '../src/assets.js';

const originals = new URL('../../res/minebomb/', import.meta.url);
const original = (name) => new Uint8Array(readFileSync(new URL(name, originals)));

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

test('every file the page or the staging script uses exists in the original package', () => {
  for (const name of [...GAME_FILES, ...SFX_FILES, BGM_FILE, ...NOTICE_FILES]) {
    assert.ok(existsSync(new URL(name, originals)), `${name} is missing from res/minebomb`);
  }
});

test('decodeVoc reads headerless PCM files', () => {
  const bytes = original('KILI.VOC');
  const { pcm, sampleRate } = decodeVoc(bytes);
  assert.equal(sampleRate, 11025);
  assert.equal(pcm.length, bytes.length);
});

test('decodeVoc reads the block layout (4-byte prefix, type 1 block, time constant)', () => {
  // EXPLOS1.VOC starts e8 32 00 00 | 01 e3 32 00 | a6 00: block size 0x32e3, time constant 0xa6
  const { pcm, sampleRate } = decodeVoc(original('EXPLOS1.VOC'));
  assert.equal(sampleRate, 11111);
  assert.equal(pcm.length, 0x32e3 - 2);
});

test('every sound effect decodes to audible-length PCM at a rate Web Audio accepts', () => {
  for (const name of SFX_FILES) {
    const { pcm, sampleRate } = decodeVoc(original(name));
    assert.ok(pcm.length > 100, `${name}: ${pcm.length} samples`);
    assert.ok(sampleRate >= 8000 && sampleRate <= 96000, `${name}: ${sampleRate} Hz`);
  }
});

test('the background music is a Scream Tracker 3 module', () => {
  const bytes = original(BGM_FILE);
  assert.equal(String.fromCharCode(...bytes.subarray(0x2c, 0x30)), 'SCRM');
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
