const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const RES_DIR = path.resolve(__dirname, '../res/minebomb');
const PUBLIC_DIR = path.resolve(__dirname, '../apps/client/public');
const TEMP_DIR = path.resolve(__dirname, '../temp_jsdos_pack');

console.log('Packaging authentic DOS Mine Bombers files into .jsdos bundle...');

if (!fs.existsSync(RES_DIR)) {
  console.error(`res/minebomb directory not found at: ${RES_DIR}`);
  process.exit(1);
}

if (fs.existsSync(TEMP_DIR)) {
  fs.rmSync(TEMP_DIR, { recursive: true, force: true });
}

fs.mkdirSync(path.join(TEMP_DIR, '.jsdos'), { recursive: true });

// Copy all files from res/minebomb into TEMP_DIR
const files = fs.readdirSync(RES_DIR);
for (const file of files) {
  const src = path.join(RES_DIR, file);
  const dst = path.join(TEMP_DIR, file);
  if (fs.statSync(src).isFile()) {
    fs.copyFileSync(src, dst);
  }
}

// Write dosbox.conf
const dosboxConf = `[dosbox]
memsize=16

[render]
aspect=true
scaler=normal2x

[cpu]
core=auto
cputype=auto
cycles=20000
cycleup=2000
cycledown=2000

[mixer]
nosound=false
rate=44100
blocksize=1024
prebuffer=20

[midi]
mpu401=intelligent
mididevice=default

[sblaster]
sbtype=sb16
sbbase=220
irq=7
dma=1
hdma=5
sbmixer=true
oplmode=auto
oplemu=default
oplrate=44100

[gus]
gus=true
gusrate=44100
gusbase=240
gusirq=5
gusdma=3
ultradir=C:\\ULTRASND

[speaker]
pcspeaker=true
pcrate=44100
tandy=auto
disney=true

[autoexec]
@echo off
mount c .
c:
cls
echo ========================================================
echo   MINE BOMBERS 3.11 (1995) - Skitso Productions
echo ========================================================
MB.EXE
`;

fs.writeFileSync(path.join(TEMP_DIR, '.jsdos', 'dosbox.conf'), dosboxConf, 'utf8');
fs.writeFileSync(path.join(TEMP_DIR, '.jsdos', 'jsdos.json'), JSON.stringify({ version: 2 }), 'utf8');

const outZip = path.join(PUBLIC_DIR, 'minebomb.zip');
const outJsdos = path.join(PUBLIC_DIR, 'minebomb.jsdos');

if (fs.existsSync(outZip)) fs.unlinkSync(outZip);
if (fs.existsSync(outJsdos)) fs.unlinkSync(outJsdos);

// Create ZIP using tar -a or zip
try {
  execSync(`tar -a -c -f "${outZip}" -C "${TEMP_DIR}" .`, { stdio: 'inherit' });
  fs.copyFileSync(outZip, outJsdos);
  console.log(`Successfully built:`);
  console.log(`  - ${outZip} (${(fs.statSync(outZip).size / 1024 / 1024).toFixed(2)} MB)`);
  console.log(`  - ${outJsdos} (${(fs.statSync(outJsdos).size / 1024 / 1024).toFixed(2)} MB)`);
} catch (err) {
  console.error('Failed to create archive with tar:', err.message);
} finally {
  fs.rmSync(TEMP_DIR, { recursive: true, force: true });
}
