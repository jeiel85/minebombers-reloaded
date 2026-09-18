import { AudioManager } from './audio.js';
import { GamepadManager } from './gamepad.js';

// WASM Key Constants
const KEY_UP = 1;
const KEY_DOWN = 2;
const KEY_LEFT = 3;
const KEY_RIGHT = 4;
const KEY_BOMB = 5;    // Space / Enter (Select, Buy)
const KEY_CHOOSE = 6;  // C / Shift (Sell / refund)
const KEY_REMOTE = 7;  // X / Ctrl
const KEY_TAB = 8;     // Tab / Q / E (Page toggle in shop)
const KEY_ESC = 9;     // Escape
const KEY_ANY = 10;

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
    this.paused = false;
    this.speed = 1.0;
    this.lastFrameTime = 0;
    this.tickInterval = 1000 / 60; // 60 FPS

    this.keyState = {
      p1: 0,
      p2: 0,
    };

    // Shared I/O buffers with WebAssembly
    this.audioPtr = 0;
    this.rumblePtr = 0;

    // Gamepad debounce state in menus
    this.lastPadButtons = 0;

    this.setupKeyboard();
    this.setupUI();
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

      // Initialize game into Title screen
      this.exports.mb_init(0, 0, 2, 2, 2);

      document.getElementById('loading').style.display = 'none';

      // Render initial Title Screen
      this.renderFramebuffer();

      this.running = true;
      requestAnimationFrame((t) => this.loop(t));
    } catch (err) {
      console.error("Initialization error:", err);
      document.getElementById('loading').textContent = "Error: " + err.message;
    }
  }

  renderFramebuffer() {
    if (!this.exports || !this.memory) return;
    const fbPtr = this.exports.mb_get_framebuffer();
    const pixels = new Uint8ClampedArray(this.memory.buffer, fbPtr, 640 * 480 * 4);
    const imgData = new ImageData(pixels, 640, 480);
    this.ctx.putImageData(imgData, 0, 0);
  }

  mapKeyCodeToWasm(code) {
    switch (code) {
      case 'ArrowUp':
      case 'KeyW':
      case 'Numpad8':
        return KEY_UP;
      case 'ArrowDown':
      case 'KeyS':
      case 'Numpad2':
      case 'Numpad5':
        return KEY_DOWN;
      case 'ArrowLeft':
      case 'KeyA':
      case 'Numpad4':
        return KEY_LEFT;
      case 'ArrowRight':
      case 'KeyD':
      case 'Numpad6':
        return KEY_RIGHT;
      case 'Space':
      case 'Enter':
      case 'NumpadEnter':
      case 'KeyZ':
        return KEY_BOMB;
      case 'KeyC':
      case 'ShiftLeft':
      case 'ShiftRight':
        return KEY_CHOOSE;
      case 'KeyX':
      case 'ControlLeft':
      case 'ControlRight':
        return KEY_REMOTE;
      case 'Tab':
      case 'KeyQ':
      case 'KeyE':
        return KEY_TAB;
      case 'Escape':
        return KEY_ESC;
      default:
        return KEY_ANY;
    }
  }

  setupKeyboard() {
    window.addEventListener('keydown', (e) => {
      if (!this.exports) return;

      this.audio.ensureContext();

      const state = this.exports.mb_get_state(); // 0: Title, 1: MainMenu, 2: Shop, 3: Battle, 4: RoundEnd

      if (state !== 3) {
        // Menu or Shop interaction
        if (e.code === 'Tab' || e.code === 'Space' || e.code.startsWith('Arrow')) {
          e.preventDefault();
        }
        const wasmKey = this.mapKeyCodeToWasm(e.code);
        const newState = this.exports.mb_handle_key(wasmKey);

        if (newState === 3) {
          this.audio.startBgm();
        }

        this.renderFramebuffer();
        return;
      }

      // In-Game Battle controls
      if (e.code === 'Escape') {
        e.preventDefault();
        this.exports.mb_handle_key(KEY_ESC);
        this.renderFramebuffer();
        return;
      }

      // P1 Keys (Arrow Keys + Space/Enter / Shift / Ctrl)
      if (e.code === 'ArrowUp') { this.keyState.p1 |= 1; e.preventDefault(); }
      if (e.code === 'ArrowDown') { this.keyState.p1 |= 2; e.preventDefault(); }
      if (e.code === 'ArrowLeft') { this.keyState.p1 |= 4; e.preventDefault(); }
      if (e.code === 'ArrowRight') { this.keyState.p1 |= 8; e.preventDefault(); }
      if (e.code === 'Space' || e.code === 'Enter' || e.code === 'KeyZ') { this.keyState.p1 |= 16; e.preventDefault(); }
      if (e.code === 'ShiftRight' || e.code === 'ShiftLeft' || e.code === 'KeyC') { this.keyState.p1 |= 32; e.preventDefault(); }
      if (e.code === 'ControlRight' || e.code === 'ControlLeft' || e.code === 'KeyX') { this.keyState.p1 |= 64; e.preventDefault(); }

      // P2 Keys (WASD + F / G / H)
      if (e.code === 'KeyW') { this.keyState.p2 |= 1; e.preventDefault(); }
      if (e.code === 'KeyS') { this.keyState.p2 |= 2; e.preventDefault(); }
      if (e.code === 'KeyA') { this.keyState.p2 |= 4; e.preventDefault(); }
      if (e.code === 'KeyD') { this.keyState.p2 |= 8; e.preventDefault(); }
      if (e.code === 'KeyF') { this.keyState.p2 |= 16; e.preventDefault(); }
      if (e.code === 'KeyG') { this.keyState.p2 |= 32; e.preventDefault(); }
      if (e.code === 'KeyH') { this.keyState.p2 |= 64; e.preventDefault(); }
    });

    window.addEventListener('keyup', (e) => {
      // P1 Keys
      if (e.code === 'ArrowUp') this.keyState.p1 &= ~1;
      if (e.code === 'ArrowDown') this.keyState.p1 &= ~2;
      if (e.code === 'ArrowLeft') this.keyState.p1 &= ~4;
      if (e.code === 'ArrowRight') this.keyState.p1 &= ~8;
      if (e.code === 'Space' || e.code === 'Enter' || e.code === 'KeyZ') this.keyState.p1 &= ~16;
      if (e.code === 'ShiftRight' || e.code === 'ShiftLeft' || e.code === 'KeyC') this.keyState.p1 &= ~32;
      if (e.code === 'ControlRight' || e.code === 'ControlLeft' || e.code === 'KeyX') this.keyState.p1 &= ~64;

      // P2 Keys
      if (e.code === 'KeyW') this.keyState.p2 &= ~1;
      if (e.code === 'KeyS') this.keyState.p2 &= ~2;
      if (e.code === 'KeyA') this.keyState.p2 &= ~4;
      if (e.code === 'KeyD') this.keyState.p2 &= ~8;
      if (e.code === 'KeyF') this.keyState.p2 &= ~16;
      if (e.code === 'KeyG') this.keyState.p2 &= ~32;
      if (e.code === 'KeyH') this.keyState.p2 &= ~64;
    });
  }

  setupUI() {
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

    // Reset / Menu button
    const restartBtn = document.getElementById('btn-restart');
    if (restartBtn) {
      restartBtn.addEventListener('click', () => {
        if (this.exports) {
          this.exports.mb_handle_key(KEY_ESC);
          this.renderFramebuffer();
        }
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
      this.audio.ensureContext();
      if (!this.exports) return;
      const state = this.exports.mb_get_state();
      if (state !== 3) {
        this.exports.mb_handle_key(KEY_ANY);
        this.renderFramebuffer();
      }
    });
  }

  handleGamepadMenuInput() {
    const pads = navigator.getGamepads ? navigator.getGamepads() : [];
    const pad = pads[0];
    if (!pad) return;

    let currentButtons = 0;
    if (pad.buttons[12] && pad.buttons[12].pressed) currentButtons |= 1; // Up
    if (pad.buttons[13] && pad.buttons[13].pressed) currentButtons |= 2; // Down
    if (pad.buttons[14] && pad.buttons[14].pressed) currentButtons |= 4; // Left
    if (pad.buttons[15] && pad.buttons[15].pressed) currentButtons |= 8; // Right
    if (pad.axes[1] < -0.5) currentButtons |= 1;
    if (pad.axes[1] > 0.5) currentButtons |= 2;
    if (pad.axes[0] < -0.5) currentButtons |= 4;
    if (pad.axes[0] > 0.5) currentButtons |= 8;

    if (pad.buttons[0] && pad.buttons[0].pressed) currentButtons |= 16; // A button (Bomb / Enter)
    if (pad.buttons[1] && pad.buttons[1].pressed) currentButtons |= 32; // B button (Choose / Sell)
    if (pad.buttons[4] && pad.buttons[4].pressed) currentButtons |= 64; // LB (Tab)
    if (pad.buttons[5] && pad.buttons[5].pressed) currentButtons |= 64; // RB (Tab)
    if (pad.buttons[9] && pad.buttons[9].pressed) currentButtons |= 128; // Start (Esc)

    // Rising edge only
    const pressed = currentButtons & ~this.lastPadButtons;
    this.lastPadButtons = currentButtons;

    if (pressed & 1) this.exports.mb_handle_key(KEY_UP);
    if (pressed & 2) this.exports.mb_handle_key(KEY_DOWN);
    if (pressed & 4) this.exports.mb_handle_key(KEY_LEFT);
    if (pressed & 8) this.exports.mb_handle_key(KEY_RIGHT);
    if (pressed & 16) {
      const newState = this.exports.mb_handle_key(KEY_BOMB);
      if (newState === 3) this.audio.startBgm();
    }
    if (pressed & 32) this.exports.mb_handle_key(KEY_CHOOSE);
    if (pressed & 64) this.exports.mb_handle_key(KEY_TAB);
    if (pressed & 128) this.exports.mb_handle_key(KEY_ESC);

    if (pressed !== 0) {
      this.renderFramebuffer();
    }
  }

  loop(timestamp) {
    if (!this.running) return;

    const delta = timestamp - this.lastFrameTime;
    const requiredInterval = this.tickInterval / this.speed;

    if (delta >= requiredInterval) {
      this.lastFrameTime = timestamp - (delta % requiredInterval);

      if (this.exports) {
        const state = this.exports.mb_get_state(); // 0: Title, 1: MainMenu, 2: Shop, 3: Battle, 4: RoundEnd

        if (state === 3) {
          // Battle simulation frame
          const padInputs = this.gamepad.getPlayerInputs();
          const p1 = this.keyState.p1 | padInputs[0];
          const p2 = this.keyState.p2 | padInputs[1];
          const p3 = padInputs[2];
          const p4 = padInputs[3];

          this.exports.mb_step(p1, p2, p3, p4);
          this.renderFramebuffer();
        } else {
          // Non-battle menus: check gamepad
          this.handleGamepadMenuInput();
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
      }
    }

    requestAnimationFrame((t) => this.loop(t));
  }
}

window.addEventListener('DOMContentLoaded', () => {
  const game = new MineBombersWeb();
  game.init();
});
