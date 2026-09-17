const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const RES_DIR = path.resolve(__dirname, '../res/minebomb');
const SHARED_OUT = path.resolve(__dirname, '../packages/shared/src/game/classicMapsData.json');
const AUDIO_OUT = path.resolve(__dirname, '../apps/client/public/assets/audio');

if (!fs.existsSync(RES_DIR)) {
  console.error('Error: res/minebomb directory not found at', RES_DIR);
  process.exit(1);
}

if (!fs.existsSync(AUDIO_OUT)) {
  fs.mkdirSync(AUDIO_OUT, { recursive: true });
}

console.log('=== Step 1: Importing 46 Official Classic Maps (.MNE) ===');

const MAP_WIDTH = 64;
const MAP_HEIGHT = 45;

function parseMNE(filename, buf) {
  const mapName = path.basename(filename, '.MNE').toUpperCase();
  const tiles = [];
  const treasures = [];
  const monsters = [];

  for (let r = 0; r < MAP_HEIGHT; r++) {
    const rowOffset = r * (MAP_WIDTH + 2);
    for (let c = 0; c < MAP_WIDTH; c++) {
      const b = buf[rowOffset + c];
      const idx = r * MAP_WIDTH + c;

      let kind = 'floor';
      if (
        b === 0x31 || // MetalWall
        (b >= 0x37 && b <= 0x39) || // Stone corners
        (b >= 0x41 && b <= 0x46) || // Stone1..4 & Boulders
        (b >= 0xAC && b <= 0xAE) || // Brick walls
        b === 0xA9 // MetalWallPlaced
      ) {
        kind = 'rock';
      } else if ((b >= 0x32 && b <= 0x36) || b === 0x9B || b === 0xA4) {
        kind = 'soil';
      } else if (b === 0x30 || b === 0x20 || b === 0x66 || b === 0xAF) {
        kind = 'floor';
      } else if (b >= 0x47 && b <= 0x56) {
        // Monster tile
        kind = 'floor';
        const isSlime = b >= 0x4F && b <= 0x52;
        monsters.push({
          tileX: c,
          tileY: r,
          kind: isSlime ? 'slime' : 'bat',
        });
      } else if (b === 0x73 || (b >= 0x92 && b <= 0x9A)) {
        // Treasure attached to soil
        kind = 'soil';
        let val = 100;
        let rarity = 'basic';
        if (b === 0x73) { val = 1000; rarity = 'rare'; } // Diamond
        else if (b === 0x9A) { val = 500; rarity = 'rare'; } // Crown
        else if (b === 0x99) { val = 350; rarity = 'rare'; } // Rubin
        else if (b === 0x98) { val = 250; rarity = 'basic'; } // Scepter
        else if (b === 0x97) { val = 200; rarity = 'basic'; } // Cross
        else if (b === 0x96) { val = 150; rarity = 'basic'; } // Bar
        else { val = 100; rarity = 'basic'; }
        treasures.push({
          id: `tr_${idx}`,
          index: idx,
          tileX: c,
          tileY: r,
          value: val,
          rarity,
        });
      } else {
        // Fallback: outer border is rock, interior is floor
        kind = (c === 0 || r === 0 || c === MAP_WIDTH - 1 || r === MAP_HEIGHT - 1) ? 'rock' : 'floor';
      }

      tiles.push(kind);
    }
  }

  // Ensure 8 safe spawn points exist across the map
  const defaultSpawns = [
    { x: 4, y: 4 },
    { x: MAP_WIDTH - 5, y: MAP_HEIGHT - 5 },
    { x: MAP_WIDTH - 5, y: 4 },
    { x: 4, y: MAP_HEIGHT - 5 },
    { x: Math.floor(MAP_WIDTH / 2), y: 4 },
    { x: Math.floor(MAP_WIDTH / 2), y: MAP_HEIGHT - 5 },
    { x: 4, y: Math.floor(MAP_HEIGHT / 2) },
    { x: MAP_WIDTH - 5, y: Math.floor(MAP_HEIGHT / 2) },
  ];

  // Clear 3x3 safe floor zone around each spawn
  for (const s of defaultSpawns) {
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const sx = s.x + dx;
        const sy = s.y + dy;
        if (sx > 0 && sy > 0 && sx < MAP_WIDTH - 1 && sy < MAP_HEIGHT - 1) {
          tiles[sy * MAP_WIDTH + sx] = 'floor';
        }
      }
    }
  }

  return {
    name: mapName,
    width: MAP_WIDTH,
    height: MAP_HEIGHT,
    tiles,
    spawns: defaultSpawns,
    treasures,
    monsters,
  };
}

const mneFiles = fs.readdirSync(RES_DIR).filter((f) => f.endsWith('.MNE')).sort();
const classicMaps = {};

for (const f of mneFiles) {
  const buf = fs.readFileSync(path.join(RES_DIR, f));
  if (buf.length >= 2970) {
    const parsed = parseMNE(f, buf);
    classicMaps[parsed.name] = parsed;
  }
}

fs.writeFileSync(SHARED_OUT, JSON.stringify(classicMaps, null, 2));
console.log(`Saved ${Object.keys(classicMaps).length} classic maps to ${SHARED_OUT}`);

console.log('\n=== Step 2: Converting Sound Blaster Sound Effects (.VOC -> .WAV) ===');

function convertVocToWav(buf) {
  let pcmData = buf;
  let sampleRate = 11025;
  if (buf[4] === 1) {
    const blockSize = buf[5] | (buf[6] << 8) | (buf[7] << 16);
    const tc = buf[8];
    sampleRate = Math.round(1000000 / (256 - tc));
    pcmData = buf.slice(10, 10 + blockSize - 2);
  }

  const wav = Buffer.alloc(44 + pcmData.length);
  wav.write('RIFF', 0);
  wav.writeUInt32LE(36 + pcmData.length, 4);
  wav.write('WAVE', 8);
  wav.write('fmt ', 12);
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20); // PCM format
  wav.writeUInt16LE(1, 22); // Mono (1 channel)
  wav.writeUInt32LE(sampleRate, 24);
  wav.writeUInt32LE(sampleRate, 28); // Byte rate
  wav.writeUInt16LE(1, 32); // Block align
  wav.writeUInt16LE(8, 34); // Bits per sample (8-bit)
  wav.write('data', 36);
  wav.writeUInt32LE(pcmData.length, 40);
  pcmData.copy(wav, 44);

  return { wav, sampleRate };
}

const vocMap = {
  'AARGH.VOC': 'sfx_aargh.wav',
  'APPLAUSE.VOC': 'sfx_applause.wav',
  'EXPLOS1.VOC': 'sfx_explos1.wav',
  'EXPLOS2.VOC': 'sfx_explos2.wav',
  'EXPLOS3.VOC': 'sfx_explos3.wav',
  'EXPLOS4.VOC': 'sfx_explos4.wav',
  'EXPLOS5.VOC': 'sfx_explos5.wav',
  'KARJAISU.VOC': 'sfx_karjaisu.wav',
  'KILI.VOC': 'sfx_kili.wav',
  'PICAXE.VOC': 'sfx_picaxe.wav',
  'PIKKUPOM.VOC': 'sfx_pikkupom.wav',
  'URETHAN.VOC': 'sfx_urethan.wav',
};

for (const [vocFile, wavName] of Object.entries(vocMap)) {
  const fullPath = path.join(RES_DIR, vocFile);
  if (fs.existsSync(fullPath)) {
    const buf = fs.readFileSync(fullPath);
    const { wav, sampleRate } = convertVocToWav(buf);
    const outPath = path.join(AUDIO_OUT, wavName);
    fs.writeFileSync(outPath, wav);
    console.log(`Converted ${vocFile} -> ${wavName} (${wav.length} bytes @ ${sampleRate} Hz)`);
  }
}

console.log('\n=== Step 3: Converting Original Tracker Music (.S3M -> .MP3) ===');

const s3mTracks = [
  { in: 'HUIPPE.S3M', out: 'bgm_huippe.mp3' },
  { in: 'OEKU.S3M', out: 'bgm_oeku.mp3' },
];

for (const t of s3mTracks) {
  const inPath = path.join(RES_DIR, t.in);
  const outPath = path.join(AUDIO_OUT, t.out);
  if (fs.existsSync(inPath)) {
    try {
      execSync(`ffmpeg -i "${inPath}" -b:a 192k -y "${outPath}"`, { stdio: 'pipe' });
      const stat = fs.statSync(outPath);
      console.log(`Converted ${t.in} -> ${t.out} (${(stat.size / 1024).toFixed(1)} KB)`);
    } catch (err) {
      console.warn(`Warning: ffmpeg conversion failed for ${t.in}:`, err.message);
    }
  }
}

console.log('\n=== All classic assets successfully imported! ===');
