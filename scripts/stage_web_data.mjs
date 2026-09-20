// Copies the original Mine Bombers files the web build needs from res/minebomb into web/data.
//
// The files are copied byte for byte (nothing is converted), together with the notice files that ship
// in the original package. web/data is git-ignored: the page fetches these files at runtime and the
// wasm module contains none of them. Run from anywhere: node scripts/stage_web_data.mjs
import { copyFileSync, existsSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { BGM_FILE, GAME_FILES, NOTICE_FILES, SFX_FILES } from '../web/src/assets.js';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const source = join(root, 'res', 'minebomb');
const target = join(root, 'web', 'data');

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
