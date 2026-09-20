// Original Mine Bombers game files.
//
// Neither this repository nor the deployed site contains any of them. The page needs the user's own
// copy (see gamedata.js) and hands the unmodified files to the game. For local development a copy
// can be staged next to the page with scripts/stage_web_data.mjs and is fetched from ./data/.

export const DATA_URL = './data/';

// Screens, font and maps that the wasm module decodes. Keep in sync with SCREEN_ASSET_FILES and
// CLASSIC_MAP_FILES in crates/mb-wasm/src/lib.rs (web/test/assets.test.mjs checks this).
export const GAME_FILES = [
  'SIKA.SPY',
  'FONTTI.FON',
  'TITLEBE.SPY',
  'MAIN3.SPY',
  'SHOPPIC.SPY',
  'OPTIONS5.SPY',
  'INFO1.SPY',
  'HALLOFFA.SPY',
  'BATTLE.MNE',
  'CASTLE.MNE',
  'JAIL.MNE',
  'LABYRINT.MNE',
  'BIOFARM.MNE',
  'OLDMINE.MNE',
  'CRUMBLE.MNE',
  'ROCKS.MNE',
];

// Sound effects in the order the wasm module numbers them (AudioEvent.effect_id).
export const SFX_FILES = [
  'KILI.VOC',
  'PICAXE.VOC',
  'EXPLOS1.VOC',
  'EXPLOS2.VOC',
  'EXPLOS3.VOC',
  'EXPLOS4.VOC',
  'EXPLOS5.VOC',
  'AARGH.VOC',
  'KARJAISU.VOC',
  'PIKKUPOM.VOC',
  'URETHAN.VOC',
  'APPLAUSE.VOC',
];

export const BGM_FILE = 'OEKU.S3M';

// The package's own notices travel with the files.
export const NOTICE_FILES = ['FILE_ID.DIZ', 'MINEENG.TXT', 'HISTORIA.TXT'];

export class MissingGameDataError extends Error {
  constructor(missing, baseUrl) {
    super(
      `Original Mine Bombers game files not found in ${baseUrl}: ${missing.join(', ')}. ` +
        'Pick your Mine Bombers 3.11 files in the page, or stage a local copy with ' +
        '"node scripts/stage_web_data.mjs <game dir>".'
    );
    this.name = 'MissingGameDataError';
    this.missing = missing;
  }
}

/**
 * Fetches the named original files.
 * @returns {Promise<Map<string, Uint8Array>>}
 * @throws {MissingGameDataError} if any file cannot be fetched
 */
export async function loadOriginalFiles(names, baseUrl = DATA_URL, fetchFn = fetch) {
  const missing = [];
  const files = new Map();
  await Promise.all(
    names.map(async (name) => {
      try {
        const res = await fetchFn(baseUrl + name);
        if (!res.ok) {
          missing.push(name);
          return;
        }
        files.set(name, new Uint8Array(await res.arrayBuffer()));
      } catch (e) {
        missing.push(name);
      }
    })
  );
  if (missing.length > 0) {
    throw new MissingGameDataError(names.filter((n) => missing.includes(n)), baseUrl);
  }
  return files;
}

/**
 * Decodes one of the game's sound effects.
 *
 * These .VOC files are not standard Creative Voice Files. Either the whole file is headerless
 * 8-bit unsigned PCM at 11025 Hz, or it has a 4-byte prefix followed by one sound-data block
 * (type 1: 24-bit size, time constant, codec byte, samples).
 * @returns {{pcm: Uint8Array, sampleRate: number}}
 */
export function decodeVoc(bytes) {
  if (bytes[4] !== 1) {
    return { pcm: bytes, sampleRate: 11025 };
  }
  const blockSize = bytes[5] | (bytes[6] << 8) | (bytes[7] << 16);
  const timeConstant = bytes[8];
  return {
    pcm: bytes.subarray(10, 10 + blockSize - 2),
    sampleRate: Math.round(1000000 / (256 - timeConstant)),
  };
}
