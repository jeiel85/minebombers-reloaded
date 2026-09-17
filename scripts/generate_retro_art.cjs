const sharp = require('sharp');
const fs = require('fs');
const path = require('path');

class PixelCanvas {
  constructor(width, height) {
    this.width = width;
    this.height = height;
    this.data = new Uint8Array(width * height * 4); // RGBA
  }

  setPixel(x, y, r, g, b, a = 255) {
    x = Math.round(x);
    y = Math.round(y);
    if (x < 0 || x >= this.width || y < 0 || y >= this.height) return;
    const idx = (y * this.width + x) * 4;
    this.data[idx] = r;
    this.data[idx + 1] = g;
    this.data[idx + 2] = b;
    this.data[idx + 3] = a;
  }

  fillRect(x, y, w, h, r, g, b, a = 255) {
    for (let dy = 0; dy < h; dy++) {
      for (let dx = 0; dx < w; dx++) {
        this.setPixel(x + dx, y + dy, r, g, b, a);
      }
    }
  }

  fillCircle(cx, cy, radius, r, g, b, a = 255) {
    const r2 = radius * radius;
    for (let dy = -radius; dy <= radius; dy++) {
      for (let dx = -radius; dx <= radius; dx++) {
        if (dx * dx + dy * dy <= r2) {
          this.setPixel(cx + dx, cy + dy, r, g, b, a);
        }
      }
    }
  }

  strokeCircle(cx, cy, radius, r, g, b, a = 255) {
    const rInner2 = (radius - 1) * (radius - 1);
    const rOuter2 = radius * radius;
    for (let dy = -radius; dy <= radius; dy++) {
      for (let dx = -radius; dx <= radius; dx++) {
        const d2 = dx * dx + dy * dy;
        if (d2 <= rOuter2 && d2 >= rInner2) {
          this.setPixel(cx + dx, cy + dy, r, g, b, a);
        }
      }
    }
  }

  async save(filePath) {
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    await sharp(this.data, {
      raw: {
        width: this.width,
        height: this.height,
        channels: 4,
      },
    })
      .png()
      .toFile(filePath);
    console.log(`Saved: ${filePath}`);
  }
}

// Helper: Hex color to RGB
function hex(hexStr) {
  const c = parseInt(hexStr.replace('#', ''), 16);
  return [(c >> 16) & 255, (c >> 8) & 255, c & 255];
}

async function generateTilesWorld() {
  const canvas = new PixelCanvas(256, 32); // 8 frames of 32x32

  // Frame 0: Floor (Dark excavated cavern)
  for (let y = 0; y < 32; y++) {
    for (let x = 0; x < 32; x++) {
      // Base floor stone
      const noise = ((x * 13 + y * 29 + (x ^ y) * 7) % 19) / 19;
      const shade = Math.floor(28 + noise * 14);
      canvas.setPixel(x, y, shade, shade - 2, shade - 4);
    }
  }
  // Scattered pebbles on floor
  const pebbles = [[6, 8], [14, 22], [22, 10], [9, 25], [26, 27], [18, 5]];
  for (const [px, py] of pebbles) {
    canvas.setPixel(px, py, 65, 60, 55);
    canvas.setPixel(px + 1, py, 75, 70, 65);
    canvas.setPixel(px, py + 1, 40, 38, 35);
  }

  // Frame 1: Soil (Rich earthy brown with organic texture)
  const f1 = 32;
  for (let y = 0; y < 32; y++) {
    for (let x = 0; x < 32; x++) {
      const n1 = ((x * 17 + y * 31 + (x * y) * 3) % 23) / 23;
      const n2 = ((x * 7 + y * 13) % 11) / 11;
      const r = Math.floor(80 + n1 * 35 + n2 * 15);
      const g = Math.floor(52 + n1 * 25 + n2 * 10);
      const b = Math.floor(30 + n1 * 15);
      canvas.setPixel(f1 + x, y, r, g, b);
    }
  }
  // Soil chunks & stones
  const dirtStones = [[f1 + 8, 12], [f1 + 20, 7], [f1 + 14, 22], [f1 + 25, 18], [f1 + 5, 27]];
  for (const [sx, sy] of dirtStones) {
    canvas.fillRect(sx, sy, 2, 2, 125, 95, 65);
    canvas.setPixel(sx + 1, sy + 1, 60, 40, 25);
  }

  // Frame 2: Rock (Solid granite boundary wall)
  const f2 = 64;
  for (let y = 0; y < 32; y++) {
    for (let x = 0; x < 32; x++) {
      const n = ((x * 23 + y * 37) % 17) / 17;
      const c = Math.floor(55 + n * 25);
      canvas.setPixel(f2 + x, y, c - 5, c, c + 8);
    }
  }
  // Beveled rock mortar bricks
  for (let y = 0; y < 32; y += 8) {
    for (let x = 0; x < 32; x++) {
      canvas.setPixel(f2 + x, y, 30, 32, 38);
      canvas.setPixel(f2 + x, Math.min(31, y + 1), 90, 100, 115);
    }
  }
  for (let y = 0; y < 32; y++) {
    const shift = (Math.floor(y / 8) % 2) * 8;
    for (let x = shift; x < 32; x += 16) {
      canvas.setPixel(f2 + x, y, 30, 32, 38);
      canvas.setPixel(f2 + Math.min(31, x + 1), y, 85, 95, 110);
    }
  }

  // Frame 3: Cracked Soil
  const f3 = 96;
  // Copy soil first
  for (let y = 0; y < 32; y++) {
    for (let x = 0; x < 32; x++) {
      const n1 = ((x * 17 + y * 31 + (x * y) * 3) % 23) / 23;
      const r = Math.floor(80 + n1 * 35);
      const g = Math.floor(52 + n1 * 25);
      const b = Math.floor(30 + n1 * 15);
      canvas.setPixel(f3 + x, y, r, g, b);
    }
  }
  // Jagged cracks
  const crackLines = [
    [f3 + 4, 4], [f3 + 8, 8], [f3 + 12, 10], [f3 + 16, 16], [f3 + 20, 20], [f3 + 25, 26],
    [f3 + 12, 10], [f3 + 14, 6], [f3 + 18, 4],
    [f3 + 16, 16], [f3 + 12, 22], [f3 + 8, 25],
  ];
  for (const [cx, cy] of crackLines) {
    canvas.fillRect(cx - 1, cy - 1, 2, 2, 25, 15, 8);
    canvas.setPixel(cx + 1, cy + 1, 130, 90, 60); // crack edge highlight
  }

  // Frame 4: Hazard
  const f4 = 128;
  for (let y = 0; y < 32; y++) {
    for (let x = 0; x < 32; x++) {
      const diag = (x + y) % 8 < 4;
      if (diag) {
        canvas.setPixel(f4 + x, y, 220, 170, 20); // yellow
      } else {
        canvas.setPixel(f4 + x, y, 35, 35, 35); // dark black/gray
      }
    }
  }
  canvas.fillRect(f4, 0, 32, 2, 80, 80, 80);
  canvas.fillRect(f4, 30, 32, 2, 40, 40, 40);
  canvas.fillRect(f4, 0, 2, 32, 80, 80, 80);
  canvas.fillRect(f4 + 30, 0, 2, 32, 40, 40, 40);

  // Frame 5: Shop Marker
  const f5 = 160;
  canvas.fillRect(f5, 0, 32, 32, 35, 30, 40);
  canvas.fillCircle(f5 + 16, 16, 11, 220, 160, 30);
  canvas.fillCircle(f5 + 16, 16, 9, 240, 190, 50);
  // Pickaxe & $ icon inside
  canvas.fillRect(f5 + 14, 10, 4, 12, 160, 100, 20);
  canvas.fillRect(f5 + 10, 12, 12, 3, 180, 120, 30);

  // Frame 6: Spawn Marker
  const f6 = 192;
  canvas.fillRect(f6, 0, 32, 32, 30, 35, 40);
  canvas.strokeCircle(f6 + 16, 16, 12, 50, 150, 220);
  canvas.fillCircle(f6 + 16, 16, 6, 40, 200, 255);

  // Frame 7: Void
  const f7 = 224;
  canvas.fillRect(f7, 0, 32, 32, 12, 14, 18);

  return canvas;
}

async function generateMiner() {
  const canvas = new PixelCanvas(128, 128); // 4 columns x 4 rows (32x32 each)

  // Colors
  const skin = [245, 195, 150];
  const skinDark = [210, 150, 110];
  const helmet = [241, 196, 15];
  const helmetHighlight = [255, 235, 59];
  const helmetShadow = [194, 157, 11];
  const lampOff = [220, 230, 240];
  const lampGlow = [0, 240, 255];
  const overalls = [41, 128, 185];
  const overallsDark = [28, 89, 128];
  const boots = [62, 39, 35];
  const bootsDark = [39, 23, 20];
  const tool = [189, 195, 199];

  // Helper to draw miner
  function drawMiner(col, row, dir, frame) {
    const ox = col * 32;
    const oy = row * 32;
    const bob = (frame === 1 || frame === 3) ? 1 : 0;
    const legOffset = (frame === 1) ? -2 : (frame === 3) ? 2 : 0;

    if (dir === 'down') {
      // Boots / Legs
      canvas.fillRect(ox + 10 + legOffset, oy + 26, 4, 5, ...boots);
      canvas.fillRect(ox + 18 - legOffset, oy + 26, 4, 5, ...boots);

      // Overalls / Body
      canvas.fillRect(ox + 9, oy + 14 + bob, 14, 12, ...overalls);
      canvas.fillRect(ox + 11, oy + 13 + bob, 10, 2, ...overallsDark); // belt
      canvas.fillRect(ox + 13, oy + 18 + bob, 6, 5, 255, 255, 255); // badge / buckle

      // Hands
      canvas.fillRect(ox + 6, oy + 17 + bob, 3, 4, ...skin);
      canvas.fillRect(ox + 23, oy + 17 + bob, 3, 4, ...skin);

      // Pickaxe handle
      canvas.fillRect(ox + 25, oy + 13 + bob, 2, 12, 140, 95, 50);
      canvas.fillRect(ox + 23, oy + 11 + bob, 6, 3, ...tool);

      // Head / Face
      canvas.fillRect(ox + 11, oy + 8 + bob, 10, 6, ...skin);
      // Eyes
      canvas.setPixel(ox + 13, oy + 10 + bob, 30, 30, 30);
      canvas.setPixel(ox + 18, oy + 10 + bob, 30, 30, 30);
      canvas.setPixel(ox + 15, oy + 13 + bob, ...skinDark); // nose

      // Helmet
      canvas.fillRect(ox + 9, oy + 3 + bob, 14, 6, ...helmet);
      canvas.fillRect(ox + 10, oy + 2 + bob, 12, 2, ...helmetHighlight);
      canvas.fillRect(ox + 8, oy + 7 + bob, 16, 2, ...helmetShadow); // brim
      // Headlamp
      canvas.fillRect(ox + 14, oy + 5 + bob, 4, 3, ...lampGlow);
      canvas.setPixel(ox + 15, oy + 6 + bob, 255, 255, 255);
    } else if (dir === 'left') {
      // Side profile facing left
      canvas.fillRect(ox + 11 + legOffset, oy + 26, 4, 5, ...boots);
      canvas.fillRect(ox + 17 - legOffset, oy + 26, 4, 5, ...bootsDark);

      // Body & Backpack
      canvas.fillRect(ox + 11, oy + 14 + bob, 10, 12, ...overalls);
      canvas.fillRect(ox + 19, oy + 15 + bob, 4, 9, 120, 80, 45); // backpack

      // Hand & tool
      canvas.fillRect(ox + 7, oy + 18 + bob, 4, 4, ...skin);
      canvas.fillRect(ox + 4, oy + 14 + bob, 3, 10, 140, 95, 50);
      canvas.fillRect(ox + 2, oy + 12 + bob, 7, 3, ...tool);

      // Face
      canvas.fillRect(ox + 10, oy + 8 + bob, 9, 6, ...skin);
      canvas.setPixel(ox + 10, oy + 10 + bob, 30, 30, 30); // eye

      // Helmet facing left
      canvas.fillRect(ox + 9, oy + 3 + bob, 12, 6, ...helmet);
      canvas.fillRect(ox + 6, oy + 7 + bob, 14, 2, ...helmetShadow); // brim jutting left
      // Headlamp pointing left
      canvas.fillRect(ox + 6, oy + 5 + bob, 3, 3, ...lampGlow);
    } else if (dir === 'right') {
      // Side profile facing right
      canvas.fillRect(ox + 11 - legOffset, oy + 26, 4, 5, ...bootsDark);
      canvas.fillRect(ox + 17 + legOffset, oy + 26, 4, 5, ...boots);

      // Body & Backpack
      canvas.fillRect(ox + 11, oy + 14 + bob, 10, 12, ...overalls);
      canvas.fillRect(ox + 9, oy + 15 + bob, 4, 9, 120, 80, 45); // backpack

      // Hand & tool
      canvas.fillRect(ox + 21, oy + 18 + bob, 4, 4, ...skin);
      canvas.fillRect(ox + 25, oy + 14 + bob, 3, 10, 140, 95, 50);
      canvas.fillRect(ox + 23, oy + 12 + bob, 7, 3, ...tool);

      // Face
      canvas.fillRect(ox + 13, oy + 8 + bob, 9, 6, ...skin);
      canvas.setPixel(ox + 21, oy + 10 + bob, 30, 30, 30); // eye

      // Helmet facing right
      canvas.fillRect(ox + 11, oy + 3 + bob, 12, 6, ...helmet);
      canvas.fillRect(ox + 12, oy + 7 + bob, 14, 2, ...helmetShadow); // brim jutting right
      // Headlamp pointing right
      canvas.fillRect(ox + 23, oy + 5 + bob, 3, 3, ...lampGlow);
    } else if (dir === 'up') {
      // Facing up / back
      canvas.fillRect(ox + 10 + legOffset, oy + 26, 4, 5, ...boots);
      canvas.fillRect(ox + 18 - legOffset, oy + 26, 4, 5, ...boots);

      // Overalls & Backpack
      canvas.fillRect(ox + 9, oy + 14 + bob, 14, 12, ...overalls);
      canvas.fillRect(ox + 11, oy + 15 + bob, 10, 10, 130, 85, 45); // big backpack
      canvas.fillRect(ox + 13, oy + 17 + bob, 6, 2, 170, 120, 70); // strap

      // Helmet from back
      canvas.fillRect(ox + 9, oy + 3 + bob, 14, 8, ...helmet);
      canvas.fillRect(ox + 10, oy + 2 + bob, 12, 2, ...helmetHighlight);
      canvas.fillRect(ox + 9, oy + 8 + bob, 14, 2, ...helmetShadow);
    }
  }

  const dirs = ['down', 'left', 'right', 'up'];
  for (let row = 0; row < 4; row++) {
    for (let col = 0; col < 4; col++) {
      drawMiner(col, row, dirs[row], col);
    }
  }

  return canvas;
}

async function generateBombs() {
  const canvas = new PixelCanvas(128, 32); // 4 frames of 32x32

  for (let f = 0; f < 4; f++) {
    const ox = f * 32;
    const pulse = f === 3 ? 1 : 0;
    const bodyColor = pulse ? [220, 60, 45] : [35, 38, 42];
    const highlight = pulse ? [255, 120, 100] : [75, 80, 90];

    // Bomb spherical body
    canvas.fillCircle(ox + 16, 18, 9, ...bodyColor);
    canvas.strokeCircle(ox + 16, 18, 9, 20, 20, 22);
    // Specular shine
    canvas.fillCircle(ox + 13, 15, 3, ...highlight);
    canvas.setPixel(ox + 13, 14, 255, 255, 255);

    // Brass collar
    canvas.fillRect(ox + 14, 7, 4, 3, 210, 160, 40);
    // Fuse wick
    canvas.setPixel(ox + 16, 6, 150, 120, 70);
    canvas.setPixel(ox + 17, 5, 150, 120, 70);
    canvas.setPixel(ox + 18, 4, 150, 120, 70);

    // Spark animation
    if (f === 1) {
      canvas.fillCircle(ox + 19, 3, 2, 255, 235, 59);
      canvas.setPixel(ox + 19, 3, 255, 255, 255);
    } else if (f === 2) {
      canvas.fillCircle(ox + 18, 4, 3, 255, 152, 0);
      canvas.fillCircle(ox + 18, 4, 1, 255, 255, 255);
    } else if (f === 3) {
      canvas.fillCircle(ox + 17, 5, 4, 255, 87, 34);
      canvas.fillCircle(ox + 17, 5, 2, 255, 255, 255);
    }
  }

  return canvas;
}

async function generateExplosions() {
  const canvas = new PixelCanvas(160, 128); // 5 cols x 4 rows

  for (let row = 0; row < 4; row++) {
    for (let col = 0; col < 5; col++) {
      const ox = col * 32;
      const oy = row * 32;
      const cx = ox + 16;
      const cy = oy + 16;

      if (col === 0) {
        // Core white-hot flash
        canvas.fillCircle(cx, cy, 7, 255, 255, 255);
        canvas.strokeCircle(cx, cy, 9, 255, 235, 59);
      } else if (col === 1) {
        // Expanding fiery blast
        canvas.fillCircle(cx, cy, 12, 255, 152, 0);
        canvas.fillCircle(cx, cy, 8, 255, 235, 59);
        canvas.fillCircle(cx, cy, 4, 255, 255, 255);
      } else if (col === 2) {
        // Full explosion cloud with red tendrils
        canvas.fillCircle(cx, cy, 14, 230, 60, 30);
        canvas.fillCircle(cx, cy, 10, 255, 160, 0);
        canvas.fillCircle(cx, cy, 5, 255, 240, 100);
        // Embers
        canvas.setPixel(cx - 11, cy - 8, 255, 235, 59);
        canvas.setPixel(cx + 10, cy - 9, 255, 200, 50);
        canvas.setPixel(cx + 8, cy + 10, 255, 100, 30);
      } else if (col === 3) {
        // Dispersing fiery smoke
        canvas.fillCircle(cx, cy, 13, 160, 50, 40, 220);
        canvas.fillCircle(cx, cy, 9, 100, 90, 85, 200);
        canvas.fillCircle(cx, cy, 4, 255, 120, 40, 240);
      } else if (col === 4) {
        // Dissipating soot & puff
        canvas.fillCircle(cx, cy, 11, 70, 65, 65, 140);
        canvas.fillCircle(cx, cy, 6, 50, 48, 48, 100);
      }
    }
  }

  return canvas;
}

async function generatePickups() {
  const canvas = new PixelCanvas(256, 32); // 8 frames of 32x32

  // Frame 0: Gold Nugget ($100)
  const f0 = 0;
  canvas.fillCircle(f0 + 16, 16, 8, 255, 193, 7);
  canvas.fillCircle(f0 + 15, 14, 5, 255, 235, 59);
  canvas.fillCircle(f0 + 13, 12, 2, 255, 255, 255); // shine
  canvas.strokeCircle(f0 + 16, 16, 8, 190, 130, 5);

  // Frame 1: Ruby Gem ($250)
  const f1 = 32;
  // Faceted diamond shape
  for (let dy = -8; dy <= 8; dy++) {
    const w = 9 - Math.abs(dy);
    for (let dx = -w; dx <= w; dx++) {
      const isTop = dy < 0 && Math.abs(dx) < 4;
      if (isTop) {
        canvas.setPixel(f1 + 16 + dx, 16 + dy, 255, 120, 130);
      } else {
        canvas.setPixel(f1 + 16 + dx, 16 + dy, 220, 30, 45);
      }
    }
  }
  canvas.fillRect(f1 + 14, 11, 3, 2, 255, 255, 255); // specular shine

  // Frame 2: Ammo Crate (+2 Bombs)
  const f2 = 64;
  canvas.fillRect(f2 + 6, 8, 20, 16, 120, 85, 45); // wood crate
  canvas.strokeCircle(f2 + 16, 16, 5, 40, 40, 40); // bomb stencil
  canvas.fillRect(f2 + 6, 8, 20, 2, 70, 50, 25);
  canvas.fillRect(f2 + 6, 22, 20, 2, 70, 50, 25);
  canvas.fillRect(f2 + 15, 13, 3, 6, 255, 193, 7);

  // Frame 3: First Aid Kit (+50 HP)
  const f3 = 96;
  canvas.fillRect(f3 + 6, 8, 20, 16, 245, 245, 245); // white case
  canvas.fillRect(f3 + 6, 8, 20, 2, 180, 180, 180);
  canvas.fillRect(f3 + 12, 6, 8, 2, 150, 150, 150); // handle
  // Red cross
  canvas.fillRect(f3 + 14, 11, 4, 10, 230, 40, 40);
  canvas.fillRect(f3 + 11, 14, 10, 4, 230, 40, 40);

  // Frame 4: Silver Ingot ($50)
  const f4 = 128;
  canvas.fillRect(f4 + 7, 12, 18, 10, 180, 190, 200);
  canvas.fillRect(f4 + 9, 10, 14, 3, 220, 230, 240); // top bevel
  canvas.fillRect(f4 + 7, 20, 18, 2, 120, 130, 140); // bottom shadow

  // Frame 5: Blue Diamond ($500)
  const f5 = 160;
  for (let dy = -8; dy <= 8; dy++) {
    const w = 8 - Math.abs(dy);
    for (let dx = -w; dx <= w; dx++) {
      if (dy < 0 && Math.abs(dx) < 3) {
        canvas.setPixel(f5 + 16 + dx, 16 + dy, 180, 240, 255);
      } else {
        canvas.setPixel(f5 + 16 + dx, 16 + dy, 0, 160, 240);
      }
    }
  }
  canvas.fillRect(f5 + 15, 11, 2, 2, 255, 255, 255);

  // Frame 6: Ancient Chest ($1000)
  const f6 = 192;
  canvas.fillRect(f6 + 6, 10, 20, 14, 140, 85, 45); // wood
  canvas.fillRect(f6 + 6, 8, 20, 5, 170, 110, 60); // curved lid
  canvas.strokeCircle(f6 + 16, 18, 2, 255, 215, 0); // gold lock
  canvas.fillRect(f6 + 6, 12, 2, 12, 255, 200, 0); // gold band L
  canvas.fillRect(f6 + 24, 12, 2, 12, 255, 200, 0); // gold band R

  // Frame 7: Rocket Pickup
  const f7 = 224;
  canvas.fillRect(f7 + 14, 8, 4, 14, 230, 235, 240); // fuselage
  canvas.fillRect(f7 + 14, 6, 4, 3, 230, 50, 40); // red nose cone
  canvas.fillRect(f7 + 11, 18, 3, 5, 230, 50, 40); // fin L
  canvas.fillRect(f7 + 18, 18, 3, 5, 230, 50, 40); // fin R

  return canvas;
}

async function generateUiIcons() {
  const canvas = new PixelCanvas(256, 32); // 8 frames of 32x32

  // Frame 0: Small Bomb
  const f0 = 0;
  canvas.fillCircle(f0 + 16, 18, 8, 40, 44, 52);
  canvas.fillCircle(f0 + 14, 15, 3, 90, 95, 105);
  canvas.fillRect(f0 + 15, 8, 2, 3, 210, 160, 40);
  canvas.setPixel(f0 + 17, 7, 255, 200, 50);

  // Frame 1: Dynamite Bundle
  const f1 = 32;
  canvas.fillRect(f1 + 10, 10, 4, 16, 215, 45, 35);
  canvas.fillRect(f1 + 14, 9, 4, 17, 235, 55, 45);
  canvas.fillRect(f1 + 18, 10, 4, 16, 215, 45, 35);
  canvas.fillRect(f1 + 9, 16, 14, 3, 30, 30, 30); // tie band
  canvas.setPixel(f1 + 16, 6, 255, 230, 80); // spark

  // Frame 2: Heavy Bomb
  const f2 = 64;
  canvas.fillCircle(f2 + 16, 17, 10, 50, 55, 65);
  canvas.fillCircle(f2 + 13, 14, 3, 110, 120, 135);
  canvas.fillRect(f2 + 11, 16, 10, 2, 230, 180, 20); // hazard stripe

  // Frame 3: Remote Bomb
  const f3 = 96;
  canvas.fillRect(f3 + 10, 12, 12, 12, 60, 65, 75);
  canvas.fillRect(f3 + 15, 6, 2, 6, 180, 180, 180); // antenna
  canvas.fillCircle(f3 + 16, 5, 2, 255, 40, 40); // blinking light

  // Frame 4: Landmine
  const f4 = 128;
  canvas.fillRect(f4 + 8, 16, 16, 6, 80, 85, 90);
  canvas.fillRect(f4 + 11, 13, 10, 3, 110, 115, 120);
  canvas.fillCircle(f4 + 16, 14, 2, 255, 40, 40); // sensor

  // Frame 5: Mini-Rocket
  const f5 = 160;
  canvas.fillRect(f5 + 14, 8, 4, 16, 240, 240, 245);
  canvas.fillRect(f5 + 14, 5, 4, 4, 230, 50, 40);
  canvas.fillRect(f5 + 10, 19, 4, 5, 230, 50, 40);
  canvas.fillRect(f5 + 18, 19, 4, 5, 230, 50, 40);

  // Frame 6: Flamethrower
  const f6 = 192;
  canvas.fillRect(f6 + 8, 14, 10, 7, 70, 75, 80); // tank
  canvas.fillRect(f6 + 17, 16, 6, 3, 160, 160, 160); // nozzle
  // Flame jet
  canvas.fillCircle(f6 + 25, 17, 4, 255, 140, 20);
  canvas.fillCircle(f6 + 24, 17, 2, 255, 240, 60);

  // Frame 7: Nuclear Warhead (Nuke)
  const f7 = 224;
  canvas.fillRect(f7 + 10, 10, 12, 16, 55, 90, 45); // military green
  canvas.fillRect(f7 + 12, 6, 8, 5, 55, 90, 45);
  // Trefoil radiation symbol
  canvas.fillCircle(f7 + 16, 18, 4, 240, 200, 20);
  canvas.fillCircle(f7 + 16, 18, 2, 30, 30, 30);

  return canvas;
}

async function generateMonsters() {
  const canvas = new PixelCanvas(128, 32); // 4 frames (32x32)

  // Frame 0: Green Slime idle
  const f0 = 0;
  canvas.fillCircle(f0 + 16, 20, 10, 46, 204, 113); // green body
  canvas.fillCircle(f0 + 16, 18, 7, 88, 214, 141); // highlight
  canvas.fillCircle(f0 + 12, 17, 3, 255, 255, 255); // eye L
  canvas.fillCircle(f0 + 19, 17, 3, 255, 255, 255); // eye R
  canvas.setPixel(f0 + 13, 17, 20, 20, 20); // pupil L
  canvas.setPixel(f0 + 20, 17, 20, 20, 20); // pupil R
  canvas.strokeCircle(f0 + 16, 20, 10, 30, 132, 73);

  // Frame 1: Green Slime squished
  const f1 = 32;
  canvas.fillCircle(f1 + 16, 23, 12, 46, 204, 113); // wider lower body
  canvas.fillCircle(f1 + 16, 22, 9, 88, 214, 141);
  canvas.fillRect(f1 + 11, 21, 3, 2, 20, 20, 20); // slit eyes
  canvas.fillRect(f1 + 18, 21, 3, 2, 20, 20, 20);

  // Frame 2: Underground Bat (Wings Up)
  const f2 = 64;
  canvas.fillCircle(f2 + 16, 16, 5, 80, 50, 95); // purple body
  canvas.fillCircle(f2 + 16, 13, 4, 95, 60, 115); // head
  // Ears
  canvas.fillRect(f2 + 13, 8, 2, 4, 95, 60, 115);
  canvas.fillRect(f2 + 17, 8, 2, 4, 95, 60, 115);
  // Red glowing eyes
  canvas.setPixel(f2 + 14, 13, 255, 50, 50);
  canvas.setPixel(f2 + 17, 13, 255, 50, 50);
  // Wings up
  canvas.fillRect(f2 + 6, 9, 7, 5, 60, 35, 75);
  canvas.fillRect(f2 + 19, 9, 7, 5, 60, 35, 75);
  canvas.fillRect(f2 + 4, 7, 4, 4, 60, 35, 75);
  canvas.fillRect(f2 + 24, 7, 4, 4, 60, 35, 75);

  // Frame 3: Underground Bat (Wings Down)
  const f3 = 96;
  canvas.fillCircle(f3 + 16, 16, 5, 80, 50, 95); // purple body
  canvas.fillCircle(f3 + 16, 13, 4, 95, 60, 115); // head
  canvas.fillRect(f3 + 13, 8, 2, 4, 95, 60, 115);
  canvas.fillRect(f3 + 17, 8, 2, 4, 95, 60, 115);
  canvas.setPixel(f3 + 14, 13, 255, 50, 50);
  canvas.setPixel(f3 + 17, 13, 255, 50, 50);
  // Wings down
  canvas.fillRect(f3 + 6, 18, 8, 6, 60, 35, 75);
  canvas.fillRect(f3 + 18, 18, 8, 6, 60, 35, 75);
  canvas.fillRect(f3 + 4, 22, 4, 4, 60, 35, 75);
  canvas.fillRect(f3 + 24, 22, 4, 4, 60, 35, 75);

  return canvas;
}

async function generateProjectiles() {
  const canvas = new PixelCanvas(96, 32); // 3 frames (32x32)

  // Frame 0: Mini-Rocket
  const f0 = 0;
  canvas.fillRect(f0 + 8, 14, 12, 4, 220, 225, 230); // body
  canvas.fillRect(f0 + 20, 13, 5, 6, 230, 50, 40); // nose cone
  canvas.fillRect(f0 + 6, 11, 4, 3, 230, 50, 40); // fin top
  canvas.fillRect(f0 + 6, 18, 4, 3, 230, 50, 40); // fin bot
  canvas.fillCircle(f0 + 4, 16, 3, 255, 180, 40); // engine flame

  // Frame 1: Flame Jet
  const f1 = 32;
  canvas.fillCircle(f1 + 16, 16, 10, 230, 70, 20); // outer orange/red
  canvas.fillCircle(f1 + 16, 16, 7, 255, 170, 30); // bright yellow
  canvas.fillCircle(f1 + 16, 16, 4, 255, 255, 200); // white-hot core

  // Frame 2: Falling Boulder / Rock
  const f2 = 64;
  canvas.fillCircle(f2 + 16, 16, 10, 90, 85, 80); // granite rock
  canvas.fillCircle(f2 + 14, 14, 7, 130, 125, 120); // highlight
  canvas.strokeCircle(f2 + 16, 16, 10, 50, 45, 40); // crust shadow
  canvas.setPixel(f2 + 12, 12, 70, 65, 60); // pit
  canvas.setPixel(f2 + 19, 18, 70, 65, 60);

  return canvas;
}

async function main() {
  const dirs = [
    path.join(__dirname, '..', 'apps', 'client', 'public', 'assets', 'gfx'),
    path.join(__dirname, '..', 'assets', 'placeholders'),
  ];

  console.log('Generating retro 1995 DOS Mine Bombers graphics...');

  const tiles = await generateTilesWorld();
  const miner = await generateMiner();
  const bombs = await generateBombs();
  const explosions = await generateExplosions();
  const pickups = await generatePickups();
  const uiIcons = await generateUiIcons();
  const monsters = await generateMonsters();
  const projectiles = await generateProjectiles();

  for (const dir of dirs) {
    await tiles.save(path.join(dir, 'tiles_world.png'));
    await miner.save(path.join(dir, 'miner.png'));
    await bombs.save(path.join(dir, 'bombs.png'));
    await explosions.save(path.join(dir, 'explosions.png'));
    await pickups.save(path.join(dir, 'pickups.png'));
    await uiIcons.save(path.join(dir, 'ui_icons.png'));
    await monsters.save(path.join(dir, 'monsters.png'));
    await projectiles.save(path.join(dir, 'projectiles.png'));
  }

  console.log('All authentic retro pixel-art spritesheets successfully generated!');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
