import { AudioManager } from './audio.js';
import { GamepadManager } from './gamepad.js';

const SHOP_ITEMS = [
  // Page 0: Classic Arsenal
  { id: 0, page: 0, name: "Small Bomb", price: 1, desc: "Standard 1-second timer explosive" },
  { id: 1, page: 0, name: "Big Bomb", price: 3, desc: "Bigger blast & high explosion damage" },
  { id: 2, page: 0, name: "Dynamite", price: 10, desc: "Dense explosive for rocks & gold" },
  { id: 4, page: 0, name: "Remote Detonator", price: 15, desc: "Detonates placed bombs with RShift / N" },
  { id: 7, page: 0, name: "Proximity Mine", price: 25, desc: "Hidden mine triggered when walked over" },
  { id: 6, page: 0, name: "Grenade", price: 300, desc: "Bouncing thrown projectile" },
  { id: 17, page: 0, name: "Small Pickaxe", price: 400, desc: "Mines dirt and stone much faster" },
  { id: 13, page: 0, name: "Plastic Explosive", price: 15, desc: "Directional shaped charge" },
  { id: 20, page: 0, name: "Teleport Device", price: 70, desc: "Instant escape to random cell" },
  { id: 24, page: 0, name: "Body Armor", price: 800, desc: "+100 extra maximum Health" },
  { id: 8, page: 0, name: "Flamethrower", price: 500, desc: "Continuous high-temp flame spray" },
  { id: 3, page: 0, name: "Atomic Bomb", price: 650, desc: "Mega screen-clearing explosion" },

  // Page 1: Special Weapons ⭐
  { id: 27, page: 1, name: "Black Hole Bomb", price: 1200, desc: "Gravitational vortex pulls enemies & explodes" },
  { id: 28, page: 1, name: "Freeze Bomb", price: 350, desc: "Freezes surrounding enemies in solid ice blocks" },
  { id: 29, page: 1, name: "Drill Drone", price: 450, desc: "Autonomous homing drone drills toward foes" }
];

class MineBombersWeb {
  constructor() {
    this.canvas = document.getElementById('screen');
    this.ctx = this.canvas.getContext('2d', { alpha: false });
    this.audio = new AudioManager();
    this.gamepad = new GamepadManager();

    this.wasm = null;
    this.memory = null;
    this.exports = null;

    this.running = false;
    this.inTitleScreen = true;
    this.paused = false;
    this.speed = 1.0;
    this.lastFrameTime = 0;
    this.tickInterval = 1000 / 60; // 60 FPS

    this.keyState = {
      p1: 0,
      p2: 0,
    };

    // Shop & Inventory State
    this.shopCart = {};
    this.shopPage = 0;
    this.playerCash = 750;
    this.customShopApplied = false;

    // Shared I/O buffers with WebAssembly
    this.audioEventBuf = null;
    this.rumbleEventBuf = null;
    this.audioPtr = 0;
    this.rumblePtr = 0;

    this.setupKeyboard();
    this.setupUI();
    this.initShopUI();
  }

  async init() {
    try {
      const response = await fetch('./pkg/mb_wasm.wasm');
      if (!response.ok) {
        throw new Error(`Failed to load wasm: ${response.statusText}`);
      }

      const wasmBytes = await response.arrayBuffer();
      const { instance } = await WebAssembly.instantiate(wasmBytes, {});

      this.wasm = instance;
      this.exports = instance.exports;
      this.memory = instance.exports.memory;

      // Preload audio
      await this.audio.preload();

      // Allocate small scratch buffers in wasm memory for audio/rumble events
      this.audioPtr = 1024 * 1024 * 3; // 3MB offset safely beyond static data
      this.rumblePtr = this.audioPtr + 64;

      document.getElementById('loading').style.display = 'none';

      // Display authentic title screen
      this.showTitleScreen();
    } catch (err) {
      console.error("Initialization error:", err);
      document.getElementById('loading').textContent = "Error: " + err.message;
    }
  }

  showTitleScreen() {
    this.inTitleScreen = true;
    this.running = false;
    this.paused = false;
    if (this.exports && this.exports.mb_render_title) {
      this.exports.mb_render_title();
      this.renderFramebuffer();
    }
    const overlay = document.getElementById('start-overlay');
    if (overlay) {
      overlay.classList.remove('hidden');
    }
    const roundEnd = document.getElementById('round-end-overlay');
    if (roundEnd) {
      roundEnd.classList.add('hidden');
    }
    const shopModal = document.getElementById('shop-modal');
    if (shopModal) {
      shopModal.classList.add('hidden');
    }
  }

  startMatch(useStarterPack = true) {
    if (!this.exports) return;

    this.inTitleScreen = false;
    this.paused = false;

    const overlay = document.getElementById('start-overlay');
    if (overlay) overlay.classList.add('hidden');
    const roundEnd = document.getElementById('round-end-overlay');
    if (roundEnd) roundEnd.classList.add('hidden');
    const shopModal = document.getElementById('shop-modal');
    if (shopModal) shopModal.classList.add('hidden');

    this.audio.ensureContext();
    this.audio.startBgm();

    // Read initial UI bot settings
    const d1 = parseInt(document.getElementById('slot-p1').value, 10);
    const d2 = parseInt(document.getElementById('slot-p2').value, 10);
    const d3 = parseInt(document.getElementById('slot-p3').value, 10);
    const d4 = parseInt(document.getElementById('slot-p4').value, 10);

    const ok = this.exports.mb_init(0, d1, d2, d3, d4);
    if (!ok) {
      console.error("Game initialization failed in WebAssembly");
      return;
    }

    if (useStarterPack) {
      this.exports.mb_equip_starter_pack(0);
      this.playerCash = this.exports.mb_get_player_cash(0);
    } else if (this.customShopApplied) {
      this.applyCartToPlayer();
    }

    this.renderFramebuffer();

    if (!this.running) {
      this.running = true;
      requestAnimationFrame((t) => this.loop(t));
    }
  }

  renderFramebuffer() {
    if (!this.exports || !this.memory) return;
    const fbPtr = this.exports.mb_get_framebuffer();
    const pixels = new Uint8ClampedArray(this.memory.buffer, fbPtr, 640 * 480 * 4);
    const imgData = new ImageData(pixels, 640, 480);
    this.ctx.putImageData(imgData, 0, 0);
  }

  // --- Shop Management ---
  initShopUI() {
    // Tabs
    document.querySelectorAll('.shop-tab').forEach((tab) => {
      tab.addEventListener('click', () => {
        document.querySelectorAll('.shop-tab').forEach((t) => t.classList.remove('active'));
        tab.classList.add('active');
        this.shopPage = parseInt(tab.getAttribute('data-page'), 10);
        this.renderShopItems();
      });
    });

    // Presets
    document.querySelectorAll('.btn-preset').forEach((btn) => {
      btn.addEventListener('click', () => {
        const preset = btn.getAttribute('data-preset');
        this.applyPreset(preset);
      });
    });

    // Close button
    const closeBtn = document.getElementById('btn-shop-close');
    if (closeBtn) {
      closeBtn.addEventListener('click', () => {
        document.getElementById('shop-modal').classList.add('hidden');
        if (!this.inTitleScreen) {
          this.paused = false;
        }
      });
    }

    // Buy & Enter Arena button
    const buyPlayBtn = document.getElementById('btn-shop-buy-play');
    if (buyPlayBtn) {
      buyPlayBtn.addEventListener('click', () => {
        const total = this.calculateCartTotal();
        if (total > this.playerCash) {
          alert("Insufficient cash! Please adjust your cart.");
          return;
        }
        this.customShopApplied = true;
        document.getElementById('shop-modal').classList.add('hidden');

        if (this.inTitleScreen || !this.running) {
          this.startMatch(false);
        } else {
          this.applyCartToPlayer();
          this.paused = false;
        }
      });
    }

    // Toolbar Shop Button
    const shopToolbarBtn = document.getElementById('btn-shop');
    if (shopToolbarBtn) {
      shopToolbarBtn.addEventListener('click', () => {
        this.openShop();
      });
    }

    // Title Screen Shop Button
    const shopTitleBtn = document.getElementById('btn-open-shop-title');
    if (shopTitleBtn) {
      shopTitleBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        this.openShop();
      });
    }

    // Round End buttons
    const nextShopBtn = document.getElementById('btn-next-shop');
    if (nextShopBtn) {
      nextShopBtn.addEventListener('click', () => {
        document.getElementById('round-end-overlay').classList.add('hidden');
        this.openShop();
      });
    }

    const nextRoundBtn = document.getElementById('btn-next-round');
    if (nextRoundBtn) {
      nextRoundBtn.addEventListener('click', () => {
        document.getElementById('round-end-overlay').classList.add('hidden');
        this.startNextRound();
      });
    }
  }

  openShop() {
    this.paused = true;
    if (this.exports) {
      const c = this.exports.mb_get_player_cash(0);
      if (c > 0) this.playerCash = c;
    } else {
      this.playerCash = 750;
    }

    if (Object.keys(this.shopCart).length === 0) {
      this.applyPreset('balanced');
    } else {
      this.renderShopItems();
    }

    const modal = document.getElementById('shop-modal');
    if (modal) modal.classList.remove('hidden');
  }

  calculateCartTotal() {
    let total = 0;
    for (const item of SHOP_ITEMS) {
      const count = this.shopCart[item.id] || 0;
      total += count * item.price;
    }
    return total;
  }

  applyPreset(preset) {
    this.shopCart = {};
    if (preset === 'balanced') {
      this.shopCart[0] = 15; // Small Bomb
      this.shopCart[1] = 5;  // Big Bomb
      this.shopCart[2] = 5;  // Dynamite
      this.shopCart[4] = 2;  // Remote
      this.shopCart[17] = 1; // Small Pickaxe
      this.shopCart[7] = 3;  // Mine
      this.shopCart[13] = 4; // Plastic
    } else if (preset === 'demo') {
      this.shopCart[0] = 20; // Small Bomb
      this.shopCart[1] = 10; // Big Bomb
      this.shopCart[2] = 12; // Dynamite
      this.shopCart[4] = 2;  // Remote
      this.shopCart[7] = 4;  // Mine
      this.shopCart[17] = 1; // Small Pickaxe
    } else if (preset === 'scifi') {
      this.shopCart[0] = 10; // Small Bomb
      this.shopCart[4] = 2;  // Remote
      this.shopCart[28] = 1; // Freeze Bomb ($350)
      this.shopCart[20] = 1; // Teleport ($70)
      this.shopCart[7] = 4;  // Mine ($100)
    }
    this.renderShopItems();
  }

  renderShopItems() {
    const container = document.getElementById('shop-items-container');
    if (!container) return;

    const totalCost = this.calculateCartTotal();
    const remaining = this.playerCash - totalCost;

    document.getElementById('shop-cash').textContent = this.playerCash;
    document.getElementById('shop-total-cost').textContent = totalCost;
    const remEl = document.getElementById('shop-remaining-cash');
    if (remEl) {
      remEl.textContent = remaining;
      remEl.style.color = remaining >= 0 ? '#2ecc71' : '#e74c3c';
    }

    const filtered = SHOP_ITEMS.filter((i) => i.page === this.shopPage);
    container.innerHTML = '';

    for (const item of filtered) {
      const qty = this.shopCart[item.id] || 0;
      const card = document.createElement('div');
      card.className = 'shop-item-card';

      card.innerHTML = `
        <div class="item-info">
          <div class="item-name">${item.name}</div>
          <div class="item-price">$${item.price} each</div>
          <div class="item-desc">${item.desc}</div>
        </div>
        <div class="item-counter">
          <button class="btn-count btn-minus" data-id="${item.id}">-</button>
          <span class="item-qty">${qty}</span>
          <button class="btn-count btn-plus" data-id="${item.id}">+</button>
        </div>
      `;

      container.appendChild(card);
    }

    // Attach +/- events
    container.querySelectorAll('.btn-minus').forEach((b) => {
      b.addEventListener('click', (e) => {
        const id = parseInt(e.target.getAttribute('data-id'), 10);
        if (this.shopCart[id] && this.shopCart[id] > 0) {
          this.shopCart[id]--;
          if (this.shopCart[id] === 0) delete this.shopCart[id];
          this.renderShopItems();
        }
      });
    });

    container.querySelectorAll('.btn-plus').forEach((b) => {
      b.addEventListener('click', (e) => {
        const id = parseInt(e.target.getAttribute('data-id'), 10);
        const item = SHOP_ITEMS.find((i) => i.id === id);
        if (item && (this.calculateCartTotal() + item.price <= this.playerCash)) {
          this.shopCart[id] = (this.shopCart[id] || 0) + 1;
          this.renderShopItems();
        }
      });
    });
  }

  applyCartToPlayer() {
    if (!this.exports) return;
    this.exports.mb_clear_player_items(0);
    for (let id = 0; id < 30; id++) {
      const cnt = this.shopCart[id] || 0;
      if (cnt > 0) {
        this.exports.mb_set_player_item(0, id, cnt);
      }
    }
    const spent = this.calculateCartTotal();
    const remaining = Math.max(0, this.playerCash - spent);
    this.exports.mb_set_player_cash(0, remaining);
    this.playerCash = remaining;
  }

  handleRoundEnd() {
    this.paused = true;
    if (!this.exports) return;

    const cash = this.exports.mb_get_player_cash(0);
    this.playerCash = cash;

    const statsEl = document.getElementById('round-end-stats');
    if (statsEl) {
      statsEl.innerHTML = `
        Round complete! You survived and extracted gold & gems.<br>
        Current Bank Account: <strong style="color: #f1c40f; font-size: 16px;">$${cash}</strong>
      `;
    }

    const overlay = document.getElementById('round-end-overlay');
    if (overlay) overlay.classList.remove('hidden');
  }

  startNextRound() {
    // Generate new random arena and proceed with current cash
    this.startMatch(false);
  }

  setupKeyboard() {
    window.addEventListener('keydown', (e) => {
      if (this.inTitleScreen) {
        if (e.code === 'Enter' || e.code === 'Space' || e.code === 'KeyZ' || e.code === 'KeyX') {
          e.preventDefault();
          this.startMatch();
          return;
        }
      }

      this.audio.ensureContext();
      this.audio.startBgm();

      // P1 Keys (Arrow Keys + RCtrl / RShift / Slash)
      if (e.code === 'ArrowUp') { this.keyState.p1 |= 1; e.preventDefault(); }
      if (e.code === 'ArrowDown') { this.keyState.p1 |= 2; e.preventDefault(); }
      if (e.code === 'ArrowLeft') { this.keyState.p1 |= 4; e.preventDefault(); }
      if (e.code === 'ArrowRight') { this.keyState.p1 |= 8; e.preventDefault(); }
      if (e.code === 'ControlRight' || e.code === 'KeyM') { this.keyState.p1 |= 16; e.preventDefault(); }
      if (e.code === 'Slash' || e.code === 'Period') { this.keyState.p1 |= 32; e.preventDefault(); }
      if (e.code === 'ShiftRight' || e.code === 'KeyN') { this.keyState.p1 |= 64; e.preventDefault(); }
      if (e.code === 'Space') { this.keyState.p1 |= 128; e.preventDefault(); }

      // P2 Keys (WASD + LShift / LCtrl / E / Q)
      if (e.code === 'KeyW') { this.keyState.p2 |= 1; e.preventDefault(); }
      if (e.code === 'KeyS') { this.keyState.p2 |= 2; e.preventDefault(); }
      if (e.code === 'KeyA') { this.keyState.p2 |= 4; e.preventDefault(); }
      if (e.code === 'KeyD') { this.keyState.p2 |= 8; e.preventDefault(); }
      if (e.code === 'ShiftLeft') { this.keyState.p2 |= 16; e.preventDefault(); }
      if (e.code === 'KeyE') { this.keyState.p2 |= 32; e.preventDefault(); }
      if (e.code === 'ControlLeft') { this.keyState.p2 |= 64; e.preventDefault(); }
      if (e.code === 'KeyQ') { this.keyState.p2 |= 128; e.preventDefault(); }
    });

    window.addEventListener('keyup', (e) => {
      if (e.code === 'ArrowUp') this.keyState.p1 &= ~1;
      if (e.code === 'ArrowDown') this.keyState.p1 &= ~2;
      if (e.code === 'ArrowLeft') this.keyState.p1 &= ~4;
      if (e.code === 'ArrowRight') this.keyState.p1 &= ~8;
      if (e.code === 'ControlRight' || e.code === 'KeyM') this.keyState.p1 &= ~16;
      if (e.code === 'Slash' || e.code === 'Period') this.keyState.p1 &= ~32;
      if (e.code === 'ShiftRight' || e.code === 'KeyN') this.keyState.p1 &= ~64;
      if (e.code === 'Space') this.keyState.p1 &= ~128;

      if (e.code === 'KeyW') this.keyState.p2 &= ~1;
      if (e.code === 'KeyS') this.keyState.p2 &= ~2;
      if (e.code === 'KeyA') this.keyState.p2 &= ~4;
      if (e.code === 'KeyD') this.keyState.p2 &= ~8;
      if (e.code === 'ShiftLeft') this.keyState.p2 &= ~16;
      if (e.code === 'KeyE') this.keyState.p2 &= ~32;
      if (e.code === 'ControlLeft') this.keyState.p2 &= ~64;
      if (e.code === 'KeyQ') this.keyState.p2 &= ~128;
    });
  }

  setupUI() {
    // Bot difficulty dropdown change events
    ['slot-p1', 'slot-p2', 'slot-p3', 'slot-p4'].forEach((id, idx) => {
      const el = document.getElementById(id);
      if (el) {
        el.addEventListener('change', () => {
          const val = parseInt(el.value, 10);
          if (this.exports) {
            this.exports.mb_set_bot_difficulty(idx, val);
          }
        });
      }
    });

    // Speed buttons
    document.querySelectorAll('.btn-speed').forEach((btn) => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('.btn-speed').forEach((b) => b.classList.remove('active'));
        btn.classList.add('active');
        this.speed = parseFloat(btn.getAttribute('data-speed'));
      });
    });

    // CRT Scanlines Toggle
    const crtBtn = document.getElementById('btn-crt');
    const container = document.getElementById('canvas-container');
    if (crtBtn && container) {
      crtBtn.addEventListener('click', () => {
        container.classList.toggle('crt');
        crtBtn.classList.toggle('active');
      });
    }

    // Quick Play Button (Title Screen)
    const startBtn = document.getElementById('btn-start-game');
    if (startBtn) {
      startBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        this.startMatch(true);
      });
    }

    const startOverlay = document.getElementById('start-overlay');
    if (startOverlay) {
      startOverlay.addEventListener('click', () => {
        this.startMatch(true);
      });
    }

    // Restart button
    const restartBtn = document.getElementById('btn-restart');
    if (restartBtn) {
      restartBtn.addEventListener('click', () => {
        this.startMatch(true);
      });
    }

    // Fullscreen button
    const fsBtn = document.getElementById('btn-fullscreen');
    if (fsBtn) {
      fsBtn.addEventListener('click', () => {
        if (!document.fullscreenElement) {
          container.requestFullscreen().catch(() => {});
        } else {
          document.exitFullscreen().catch(() => {});
        }
      });
    }

    // Mute button
    const muteBtn = document.getElementById('btn-mute');
    if (muteBtn) {
      muteBtn.addEventListener('click', () => {
        const isMuted = this.audio.toggleMute();
        muteBtn.textContent = isMuted ? '🔇 Unmute' : '🔊 Sound';
      });
    }

    // Click canvas
    this.canvas.addEventListener('click', () => {
      if (this.inTitleScreen) {
        this.startMatch(true);
      } else {
        this.audio.ensureContext();
        this.audio.startBgm();
      }
    });
  }

  loop(timestamp) {
    if (!this.running) return;

    const delta = timestamp - this.lastFrameTime;
    const requiredInterval = this.tickInterval / this.speed;

    if (delta >= requiredInterval) {
      this.lastFrameTime = timestamp - (delta % requiredInterval);

      if (!this.paused) {
        // Collect Gamepad inputs
        const padInputs = this.gamepad.getPlayerInputs();

        // Merge Keyboard + Gamepad
        const p1 = this.keyState.p1 | padInputs[0];
        const p2 = this.keyState.p2 | padInputs[1];
        const p3 = padInputs[2];
        const p4 = padInputs[3];

        // Step WASM simulation
        const stepRes = this.exports.mb_step(p1, p2, p3, p4);
        if (stepRes === 2) {
          this.handleRoundEnd();
        }

        // Process audio events from WASM
        const memI32 = new Int32Array(this.memory.buffer);
        const memF32 = new Float32Array(this.memory.buffer);
        const audioOffset = this.audioPtr >> 2;
        const rumbleOffset = this.rumblePtr >> 2;

        while (this.exports.mb_get_audio_event(this.audioPtr) === 1) {
          const effId = memI32[audioOffset];
          const freq = memI32[audioOffset + 1];
          const pan = memI32[audioOffset + 2] / 100.0;
          this.audio.playSfx(effId, freq, pan);
        }

        // Process rumble events from WASM
        while (this.exports.mb_get_rumble_event(this.rumblePtr) === 1) {
          const pIdx = Math.round(memF32[rumbleOffset]);
          const intensity = memF32[rumbleOffset + 1];
          const duration = memF32[rumbleOffset + 2];
          this.gamepad.rumble(pIdx, intensity, duration);
        }

        // Render Framebuffer
        this.renderFramebuffer();
      }
    }

    requestAnimationFrame((t) => this.loop(t));
  }
}

window.addEventListener('DOMContentLoaded', () => {
  const game = new MineBombersWeb();
  game.init();
});
