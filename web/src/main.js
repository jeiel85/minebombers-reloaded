import { AudioManager } from './audio.js';
import { GamepadManager } from './gamepad.js';

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
    this.audioEventBuf = null;
    this.rumbleEventBuf = null;
    this.audioPtr = 0;
    this.rumblePtr = 0;

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

      // Initialize audio
      await this.audio.preload();

      // Read initial UI bot settings
      const d1 = parseInt(document.getElementById('slot-p1').value, 10);
      const d2 = parseInt(document.getElementById('slot-p2').value, 10);
      const d3 = parseInt(document.getElementById('slot-p3').value, 10);
      const d4 = parseInt(document.getElementById('slot-p4').value, 10);

      const ok = this.exports.mb_init(0, d1, d2, d3, d4);
      if (!ok) {
        throw new Error("Game initialization failed in WebAssembly");
      }

      // Allocate small scratch buffers in wasm memory for audio/rumble events
      // (Using stack/static pointers via simple malloc or heap offset)
      // Since wasm memory starts with data, we allocate at high offset or use heap
      this.audioPtr = 1024 * 1024 * 3; // 3MB offset safely beyond static data
      this.rumblePtr = this.audioPtr + 64;

      this.running = true;
      document.getElementById('loading').style.display = 'none';

      // Start loop
      requestAnimationFrame((t) => this.loop(t));
    } catch (err) {
      console.error("Initialization error:", err);
      document.getElementById('loading').textContent = "Error: " + err.message;
    }
  }

  setupKeyboard() {
    window.addEventListener('keydown', (e) => {
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

    // Restart button
    const restartBtn = document.getElementById('btn-restart');
    if (restartBtn) {
      restartBtn.addEventListener('click', () => {
        const d1 = parseInt(document.getElementById('slot-p1').value, 10);
        const d2 = parseInt(document.getElementById('slot-p2').value, 10);
        const d3 = parseInt(document.getElementById('slot-p3').value, 10);
        const d4 = parseInt(document.getElementById('slot-p4').value, 10);
        this.exports.mb_init(0, d1, d2, d3, d4);
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

    // Click canvas to resume audio
    this.canvas.addEventListener('click', () => {
      this.audio.ensureContext();
      this.audio.startBgm();
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
        this.exports.mb_step(p1, p2, p3, p4);

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
        const fbPtr = this.exports.mb_get_framebuffer();
        const pixels = new Uint8ClampedArray(this.memory.buffer, fbPtr, 640 * 480 * 4);
        const imgData = new ImageData(pixels, 640, 480);
        this.ctx.putImageData(imgData, 0, 0);
      }
    }

    requestAnimationFrame((t) => this.loop(t));
  }
}

window.addEventListener('DOMContentLoaded', () => {
  const game = new MineBombersWeb();
  game.init();
});
