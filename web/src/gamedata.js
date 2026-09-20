// Getting the original Mine Bombers 3.11 files into the page.
//
// The repository ships none of them. Like OpenRCT2 with RollerCoaster Tycoon 2, the game needs the
// user's own copy: the user picks the files, a folder or the original ZIP once, and the page keeps
// the ~30 files it needs in this browser (IndexedDB). They are never uploaded anywhere.
import { BGM_FILE, DATA_URL, GAME_FILES, SFX_FILES, loadOriginalFiles } from './assets.js';

export const REQUIRED_FILES = [...GAME_FILES, ...SFX_FILES, BGM_FILE];

const DB_NAME = 'mine-bombers-game-files';
const STORE = 'files';
const KEY = 'mb311';

/** "MB311/Sub/aargh.voc" -> "AARGH.VOC" */
export function baseName(path) {
  return path.split(/[\\/]/).pop().toUpperCase();
}

/**
 * Keeps only the required files, under their upper-case names.
 * @param {Iterable<[string, Uint8Array]>} entries (path, bytes) pairs
 * @returns {{files: Map<string, Uint8Array>, missing: string[]}}
 */
export function pickRequired(entries) {
  const wanted = new Set(REQUIRED_FILES);
  const files = new Map();
  for (const [path, bytes] of entries) {
    const name = baseName(path);
    if (wanted.has(name) && !files.has(name)) {
      files.set(name, bytes);
    }
  }
  return { files, missing: REQUIRED_FILES.filter((name) => !files.has(name)) };
}

export function describeMissing(missing, limit = 6) {
  const shown = missing.slice(0, limit).join(', ');
  const more = missing.length > limit ? ` and ${missing.length - limit} more` : '';
  return `Not a complete Mine Bombers 3.11 copy: ${missing.length} required file(s) missing (${shown}${more}).`;
}

async function inflateRaw(data) {
  const stream = new Blob([data]).stream().pipeThrough(new DecompressionStream('deflate-raw'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

/**
 * Extracts the required files from a ZIP archive (stored or deflated entries, no ZIP64).
 * Folders inside the archive do not matter, only the file names.
 * @param {Uint8Array} bytes
 * @returns {Promise<Map<string, Uint8Array>>}
 */
export async function readZip(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let eocd = -1;
  for (let i = bytes.length - 22; i >= Math.max(0, bytes.length - 22 - 0xffff); i--) {
    if (view.getUint32(i, true) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error('This is not a ZIP file.');
  const count = view.getUint16(eocd + 10, true);
  let offset = view.getUint32(eocd + 16, true);
  if (count === 0xffff || offset === 0xffffffff) throw new Error('ZIP64 archives are not supported.');

  const wanted = new Set(REQUIRED_FILES);
  const out = new Map();
  for (let n = 0; n < count; n++) {
    if (view.getUint32(offset, true) !== 0x02014b50) throw new Error('The ZIP file is damaged.');
    const method = view.getUint16(offset + 10, true);
    const compressedSize = view.getUint32(offset + 20, true);
    const nameLength = view.getUint16(offset + 28, true);
    const extraLength = view.getUint16(offset + 30, true);
    const commentLength = view.getUint16(offset + 32, true);
    const localOffset = view.getUint32(offset + 42, true);
    const name = baseName(new TextDecoder().decode(bytes.subarray(offset + 46, offset + 46 + nameLength)));
    offset += 46 + nameLength + extraLength + commentLength;
    if (!wanted.has(name) || out.has(name)) continue;

    if (view.getUint32(localOffset, true) !== 0x04034b50) throw new Error(`The ZIP entry ${name} is damaged.`);
    const dataStart = localOffset + 30 + view.getUint16(localOffset + 26, true) + view.getUint16(localOffset + 28, true);
    const data = bytes.subarray(dataStart, dataStart + compressedSize);
    if (method === 0) {
      out.set(name, data.slice());
    } else if (method === 8) {
      out.set(name, await inflateRaw(data));
    } else {
      throw new Error(`The ZIP entry ${name} uses an unsupported compression method (${method}).`);
    }
  }
  return out;
}

/**
 * Collects the required files from what the user picked: single files, a folder, and/or ZIP archives.
 * @param {Iterable<File>} fileList
 */
export async function collectFromFiles(fileList) {
  const wanted = new Set(REQUIRED_FILES);
  const entries = [];
  for (const file of fileList) {
    if (/\.zip$/i.test(file.name)) {
      entries.push(...(await readZip(new Uint8Array(await file.arrayBuffer()))));
    } else if (wanted.has(baseName(file.name))) {
      entries.push([file.name, new Uint8Array(await file.arrayBuffer())]);
    }
  }
  return pickRequired(entries);
}

// ---- Browser storage. It is only a convenience, so every failure just means "nothing stored". ----

function openDb() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function withStore(mode, action) {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(STORE, mode);
      const request = action(tx.objectStore(STORE));
      tx.oncomplete = () => resolve(request.result);
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** @returns {Promise<Map<string, Uint8Array> | null>} the stored copy, if complete */
export async function loadStored() {
  try {
    const stored = await withStore('readonly', (store) => store.get(KEY));
    if (!(stored instanceof Map)) return null;
    const { files, missing } = pickRequired(stored);
    return missing.length === 0 ? files : null;
  } catch (e) {
    return null;
  }
}

export async function saveStored(files) {
  try {
    await withStore('readwrite', (store) => store.put(files, KEY));
    return true;
  } catch (e) {
    console.warn('Could not keep the game files in this browser:', e);
    return false;
  }
}

export async function clearStored() {
  try {
    await withStore('readwrite', (store) => store.delete(KEY));
  } catch (e) {
    console.warn('Could not remove the stored game files:', e);
  }
}

export function isLocalHost(host = globalThis.location?.hostname) {
  return ['localhost', '127.0.0.1', '[::1]', '::1'].includes(host);
}

/** A copy staged next to the page by scripts/stage_web_data.mjs (local development). */
export async function loadStaged(fetchFn = fetch) {
  try {
    const probe = await fetchFn(DATA_URL + REQUIRED_FILES[0]);
    if (!probe.ok) return null;
    return await loadOriginalFiles(REQUIRED_FILES, DATA_URL, fetchFn);
  } catch (e) {
    return null;
  }
}

/**
 * The original files, from the first source that has them: this browser's storage, a staged copy,
 * or the user through `promptUser` (which must resolve with a complete file map).
 * @returns {Promise<{files: Map<string, Uint8Array>, source: 'stored' | 'staged' | 'user'}>}
 */
export async function acquireGameFiles(promptUser) {
  const stored = await loadStored();
  if (stored) return { files: stored, source: 'stored' };
  // A staged copy only exists on a developer's machine; visitors of the site should not see a 404
  const staged = isLocalHost() ? await loadStaged() : null;
  if (staged) return { files: staged, source: 'staged' };
  const files = await promptUser();
  await saveStored(files);
  return { files, source: 'user' };
}
