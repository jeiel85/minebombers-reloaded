// Local development only: copies the original Mine Bombers files the web build needs from your own
// copy of the game into web/data, so the page starts without asking for them.
//
//   node scripts/stage_web_data.mjs <Mine Bombers 3.11 folder>     (or set MB_GAME_DIR)
//
// The repository contains no original files and web/data is git-ignored; the deployed site asks each
// visitor for their own copy instead. Files are copied byte for byte, nothing is converted.
import { copyFileSync, existsSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { BGM_FILE, GAME_FILES, NOTICE_FILES, SFX_FILES } from '../web/src/assets.js';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const given = process.argv[2] || process.env.MB_GAME_DIR;
const source = given ? resolve(given) : join(root, 'res', 'minebomb');
const target = join(root, 'web', 'data');

if (!existsSync(source)) {
  console.error(`Game folder not found: ${source}`);
  console.error('Usage: node scripts/stage_web_data.mjs <folder with your Mine Bombers 3.11 files> (or set MB_GAME_DIR)');
  process.exit(1);
}

const names = [...GAME_FILES, ...SFX_FILES, BGM_FILE, ...NOTICE_FILES];
const missing = names.filter((name) => !existsSync(join(source, name)));
if (missing.length > 0) {
  console.error(`Missing from ${source}: ${missing.join(', ')}`);
  process.exit(1);
}

rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });
for (const name of names) {
  copyFileSync(join(source, name), join(target, name));
}
console.log(`Staged ${names.length} original game files in ${target}`);
