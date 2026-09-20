import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateRawSync } from 'node:zlib';
import {
  REQUIRED_FILES,
  acquireGameFiles,
  baseName,
  collectFromFiles,
  describeMissing,
  isLocalHost,
  loadStaged,
  pickRequired,
  readZip,
} from '../src/gamedata.js';

// Minimal ZIP writer: entries are { name, data: Buffer, method: 0 (stored) | 8 (deflate) }
function buildZip(entries) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const { name, data, method, rawBody } of entries) {
    const nameBuf = Buffer.from(name);
    const body = rawBody ?? (method === 8 ? deflateRawSync(data) : data);
    const local = Buffer.alloc(30 + nameBuf.length);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(method, 8);
    local.writeUInt32LE(body.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBuf.length, 26);
    nameBuf.copy(local, 30);
    const central = Buffer.alloc(46 + nameBuf.length);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(method, 10);
    central.writeUInt32LE(body.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBuf.length, 28);
    central.writeUInt32LE(offset, 42);
    nameBuf.copy(central, 46);
    locals.push(local, body);
    centrals.push(central);
    offset += local.length + body.length;
  }
  const directory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return new Uint8Array(Buffer.concat([...locals, directory, end]));
}

const bytesOf = (text) => Buffer.from(text);
const fakeFile = (name, content) => ({ name, arrayBuffer: async () => new Uint8Array(bytesOf(content)).buffer });

test('baseName drops folders and upper-cases', () => {
  assert.equal(baseName('MB311/data/aargh.voc'), 'AARGH.VOC');
  assert.equal(baseName('C:\\GAMES\\MB\\Main3.spy'), 'MAIN3.SPY');
  assert.equal(baseName('KILI.VOC'), 'KILI.VOC');
  assert.equal(baseName('folder/'), '');
});

test('pickRequired keeps only required files, upper-cased, and reports what is missing', () => {
  const entries = [
    ['game/sika.spy', new Uint8Array([1])],
    ['game/MB.EXE', new Uint8Array([2])],
    ['game/kili.voc', new Uint8Array([3])],
    ['other/SIKA.SPY', new Uint8Array([9])], // the first one wins
  ];
  const { files, missing } = pickRequired(entries);
  assert.deepEqual([...files.keys()].sort(), ['KILI.VOC', 'SIKA.SPY']);
  assert.deepEqual([...files.get('SIKA.SPY')], [1]);
  assert.equal(missing.length, REQUIRED_FILES.length - 2);
  assert.ok(!missing.includes('SIKA.SPY'));
});

test('describeMissing shortens long lists', () => {
  assert.match(describeMissing(['A', 'B']), /2 required file\(s\) missing \(A, B\)/);
  const many = describeMissing(['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'], 3);
  assert.match(many, /8 required file\(s\) missing \(A, B, C and 5 more\)/);
});

test('readZip extracts stored and deflated entries and ignores folders and other files', async () => {
  const kili = bytesOf('kili sound');
  const sika = Buffer.alloc(5000, 7); // compresses well
  const zip = buildZip([
    { name: 'MB311/', data: Buffer.alloc(0), method: 0 },
    { name: 'MB311/kili.voc', data: kili, method: 0 },
    { name: 'MB311/SIKA.SPY', data: sika, method: 8 },
    { name: 'MB311/MB.EXE', data: bytesOf('not needed'), method: 0 },
  ]);
  const files = await readZip(zip);
  assert.deepEqual([...files.keys()].sort(), ['KILI.VOC', 'SIKA.SPY']);
  assert.deepEqual(Buffer.from(files.get('KILI.VOC')), kili);
  assert.deepEqual(Buffer.from(files.get('SIKA.SPY')), sika);
});

test('readZip never touches entries it does not need', async () => {
  // A damaged deflate stream for a file the game does not use must not matter
  const zip = buildZip([
    { name: 'JUNK.BIN', data: bytesOf('x'), method: 8, rawBody: Buffer.from([0xff, 0xff, 0xff]) },
    { name: 'CASTLE.MNE', data: bytesOf('map'), method: 0 },
  ]);
  const files = await readZip(zip);
  assert.deepEqual([...files.keys()], ['CASTLE.MNE']);
});

test('readZip explains what is wrong', async () => {
  await assert.rejects(readZip(new Uint8Array([1, 2, 3, 4])), /not a ZIP file/);
  const unsupported = buildZip([{ name: 'SIKA.SPY', data: bytesOf('x'), method: 12 }]);
  await assert.rejects(readZip(unsupported), /SIKA\.SPY.*unsupported compression method \(12\)/);
  const truncated = buildZip([{ name: 'SIKA.SPY', data: bytesOf('x'), method: 0 }]);
  truncated[0] = 0; // break the first local header
  await assert.rejects(readZip(truncated), /SIKA\.SPY is damaged/);
});

test('collectFromFiles reads single files and ZIPs but never opens unrelated files', async () => {
  const zip = buildZip([{ name: 'ROCKS.MNE', data: bytesOf('rocks'), method: 8 }]);
  const picked = [
    fakeFile('kili.voc', 'kili'),
    { name: 'bundle.ZIP', arrayBuffer: async () => zip.buffer.slice(zip.byteOffset, zip.byteOffset + zip.byteLength) },
    { name: 'MB.EXE', arrayBuffer: async () => assert.fail('an unrelated file must not be read') },
  ];
  const { files, missing } = await collectFromFiles(picked);
  assert.deepEqual([...files.keys()].sort(), ['KILI.VOC', 'ROCKS.MNE']);
  assert.ok(missing.includes('BATTLE.MNE'));
  assert.ok(!missing.includes('ROCKS.MNE'));
});

test('acquireGameFiles falls back to asking the user when nothing is stored or staged', async () => {
  const provided = new Map(REQUIRED_FILES.map((name) => [name, new Uint8Array([1])]));
  let asked = 0;
  const warn = console.warn;
  console.warn = () => {}; // there is no IndexedDB in Node, so saving is expected to fail quietly
  try {
    const result = await acquireGameFiles(async () => {
      asked++;
      return provided;
    });
    assert.equal(asked, 1);
    assert.equal(result.source, 'user');
    assert.equal(result.files, provided);
  } finally {
    console.warn = warn;
  }
});

test('a staged copy is only looked for on local addresses', () => {
  assert.ok(isLocalHost('localhost'));
  assert.ok(isLocalHost('127.0.0.1'));
  assert.ok(!isLocalHost('jeiel85.github.io'));
  assert.ok(!isLocalHost(undefined));
});

test('loadStaged gives up quietly when nothing is staged and returns the files when it is', async () => {
  assert.equal(await loadStaged(async () => ({ ok: false, status: 404 })), null);
  assert.equal(await loadStaged(async () => { throw new TypeError('offline'); }), null);
  const complete = async () => ({ ok: true, arrayBuffer: async () => new Uint8Array([5]).buffer });
  const files = await loadStaged(complete);
  assert.deepEqual([...files.keys()], REQUIRED_FILES);
  // A partial copy is treated as no copy, so the page asks instead of failing half way
  const partial = async (url) => (url.endsWith('MAIN3.SPY') ? { ok: false, status: 404 } : complete());
  assert.equal(await loadStaged(partial), null);
});

// ---- against a real ZIP of the original game, for developers who have one ----

const realZip = process.env.MB_GAME_ZIP;
test('readZip finds every required file in the real Mine Bombers 3.11 ZIP', {
  skip: !(realZip && existsSync(realZip)) && 'needs your own Mine Bombers 3.11 ZIP (set MB_GAME_ZIP)',
}, async () => {
  const files = await readZip(new Uint8Array(readFileSync(realZip)));
  const { missing } = pickRequired(files);
  assert.deepEqual(missing, []);
  const dir = process.env.MB_GAME_DIR || fileURLToPath(new URL('../../res/minebomb/', import.meta.url));
  if (existsSync(join(dir, 'KILI.VOC'))) {
    assert.deepEqual(Buffer.from(files.get('KILI.VOC')), readFileSync(join(dir, 'KILI.VOC')));
  }
});
