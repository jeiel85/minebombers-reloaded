import { AudioManager } from './audio.js';
import { GAME_FILES } from './assets.js';
import { acquireGameFiles, clearStored } from './gamedata.js';
import { promptForGameFiles } from './gamedata-ui.js';
import { GamepadManager } from './gamepad.js';
import { NetplayManager } from './network.js';
import { TouchController } from './touch.js';
import { setupChrome } from './ui.js';

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
    this.netplay = new NetplayManager();
    this.touch = new TouchController(this);

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
    this.setupNetplay();
    setupChrome();
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

      // The user's own copy of the original game; neither this site nor the wasm module contains it
      const { files, source } = await acquireGameFiles(promptForGameFiles);
      if (source !== 'staged') {
        const forget = document.getElementById('btn-forget-data');
        forget.hidden = false;
        forget.addEventListener('click', async () => {
          await clearStored();
          location.reload();
        });
      }
      for (const name of GAME_FILES) {
        this.registerGameFile(name, files.get(name));
      }
      this.audio.preload(files);

      // Scratch buffers for audio/rumble events, allocated by the module so they never overlap its heap
      this.audioPtr = this.allocScratch(64);
      this.rumblePtr = this.allocScratch(64);

      // Initialize game into Title screen
      if (this.exports.mb_init(0, 0, 2, 2, 2) !== 1) {
        if (source !== 'staged') await clearStored(); // do not keep a copy that does not work
        throw new Error(
          'The game files could not be read. Are they from Mine Bombers 3.11? Reload the page to choose them again.'
        );
      }

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

  // Copies bytes into a fresh buffer inside the wasm module and returns its address.
  writeToWasm(bytes) {
    const ptr = this.exports.mb_alloc(bytes.length) >>> 0;
    // Build the view after the allocation: it may have grown (and detached) the old memory buffer
    new Uint8Array(this.memory.buffer, ptr, bytes.length).set(bytes);
    return ptr;
  }

  // Hands one original game file to the wasm module (it takes ownership of both buffers)
  registerGameFile(name, bytes) {
    const nameBytes = new TextEncoder().encode(name);
    const namePtr = this.writeToWasm(nameBytes);
    const dataPtr = this.writeToWasm(bytes);
    if (this.exports.mb_asset_register(namePtr, nameBytes.length, dataPtr, bytes.length) !== 1) {
      throw new Error(`Could not register game file ${name}`);
    }
  }

  // 16-byte aligned scratch space that lives as long as the page
  allocScratch(size) {
    return ((this.exports.mb_alloc(size + 15) >>> 0) + 15) & ~15;
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

      const state = this.exports.mb_get_state(); // 0: Title, 1: MainMenu, 2: Options, 3: Info, 4: Shop, 5: Battle, 6: RoundEnd, 7: HallOfFame

      if (state !== 5) {
        // Menu or Shop interaction
        if (e.code === 'Tab' || e.code === 'Space' || e.code.startsWith('Arrow') || e.code.startsWith('Shift') || e.code === 'KeyC') {
          e.preventDefault();
        }
        const wasmKey = this.mapKeyCodeToWasm(e.code);

        // Prevent rapid repeated keydown events from OS auto-repeat for discrete actions like Sell, Buy, Tab, Esc
        if (e.repeat && (wasmKey === KEY_CHOOSE || wasmKey === KEY_BOMB || wasmKey === KEY_TAB || wasmKey === KEY_ESC)) {
          return;
        }

        const newState = this.exports.mb_handle_key(wasmKey);

        if (newState === 5) {
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

    window.addEventListener('blur', () => {
      this.keyState.p1 = 0;
      this.keyState.p2 = 0;
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
        muteBtn.textContent = isMuted ? '🔇' : '🔊';
        muteBtn.title = muteBtn.ariaLabel = isMuted ? 'Unmute' : 'Sound';
      });
    }

    // Click canvas
    this.canvas.addEventListener('click', () => {
      this.audio.ensureContext();
      if (!this.exports) return;
      const state = this.exports.mb_get_state();
      if (state !== 5) {
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
      if (newState === 5) this.audio.startBgm();
    }
    if (pressed & 32) this.exports.mb_handle_key(KEY_CHOOSE);
    if (pressed & 64) this.exports.mb_handle_key(KEY_TAB);
    if (pressed & 128) this.exports.mb_handle_key(KEY_ESC);

    if (pressed !== 0) {
      this.renderFramebuffer();
    }
  }

  setupNetplay() {
    const modal = document.getElementById('netplay-modal');
    const btnNetplay = document.getElementById('btn-netplay');
    const btnClose = document.getElementById('netplay-close');
    const tabHost = document.getElementById('tab-host');
    const tabJoin = document.getElementById('tab-join');
    const tabLan = document.getElementById('tab-lan');
    const panelHost = document.getElementById('panel-host');
    const panelJoin = document.getElementById('panel-join');
    const panelLan = document.getElementById('panel-lan');
    const hostCodeInput = document.getElementById('host-code-input');
    const btnCopyCode = document.getElementById('btn-copy-code');
    const hostPlayerList = document.getElementById('host-player-list');
    const btnStartMatch = document.getElementById('btn-start-net-match');
    const joinCodeInput = document.getElementById('join-code-input');
    const btnConnectCode = document.getElementById('btn-connect-code');
    const joinStatus = document.getElementById('join-status');
    const btnRefreshLan = document.getElementById('btn-refresh-lan');
    const statusText = document.getElementById('netplay-status-text');
    const pingDisplay = document.getElementById('netplay-ping');

    const switchTab = (activeTab, activePanel) => {
      [tabHost, tabJoin, tabLan].forEach(t => t && t.classList.remove('active'));
      [panelHost, panelJoin, panelLan].forEach(p => p && (p.style.display = 'none'));
      if (activeTab) activeTab.classList.add('active');
      if (activePanel) activePanel.style.display = 'block';
    };

    if (btnNetplay) {
      btnNetplay.addEventListener('click', () => {
        if (modal) {
          modal.style.display = 'flex';
          if (this.netplay.state === 'DISCONNECTED') {
            const mode = document.getElementById('net-game-mode')?.value || 'Survival Horde';
            this.netplay.hostRoom(mode).then(code => {
              if (hostCodeInput) hostCodeInput.value = code;
            }).catch(() => {});
          }
        }
      });
    }

    if (btnClose) {
      btnClose.addEventListener('click', () => {
        if (modal) modal.style.display = 'none';
      });
    }

    if (tabHost) tabHost.addEventListener('click', () => switchTab(tabHost, panelHost));
    if (tabJoin) tabJoin.addEventListener('click', () => switchTab(tabJoin, panelJoin));
    if (tabLan) tabLan.addEventListener('click', () => switchTab(tabLan, panelLan));

    if (btnCopyCode) {
      btnCopyCode.addEventListener('click', () => {
        if (this.netplay.roomCode) {
          navigator.clipboard.writeText(this.netplay.roomCode).then(() => {
            btnCopyCode.textContent = '✓ Copied!';
            setTimeout(() => { btnCopyCode.textContent = '📋 Copy'; }, 2000);
          });
        }
      });
    }

    if (btnStartMatch) {
      btnStartMatch.addEventListener('click', () => {
        if (!this.netplay.isHost) return;
        const mode = document.getElementById('net-game-mode')?.value || 'Survival Horde';
        const seed = Math.floor(Math.random() * 1000000);
        const biome = Math.floor(Math.random() * 4);
        this.netplay.startMatch(seed, mode, biome);
      });
    }

    if (btnConnectCode) {
      btnConnectCode.addEventListener('click', () => {
        const code = joinCodeInput?.value.trim().toUpperCase();
        if (!code || code.length < 4) {
          if (joinStatus) joinStatus.textContent = 'Please enter a valid room code (e.g. 7X4K29).';
          return;
        }
        if (joinStatus) joinStatus.textContent = `Connecting to [${code}]...`;
        this.netplay.joinRoom(code).then(() => {
          if (joinStatus) joinStatus.textContent = 'Connected! Waiting for host to start match...';
        }).catch(err => {
          if (joinStatus) joinStatus.textContent = `Failed: ${err.message || err}`;
        });
      });
    }

    if (btnRefreshLan) {
      btnRefreshLan.addEventListener('click', () => {
        this.renderLanRooms(Array.from(this.netplay.lanRooms.values()));
      });
    }

    // Netplay Callbacks
    this.netplay.onStatusChange = (msg, state) => {
      if (statusText) statusText.textContent = msg;
      if (btnNetplay) {
        if (state === 'CONNECTED' || state === 'IN_GAME') {
          btnNetplay.style.background = '#27ae60';
          btnNetplay.style.color = '#fff';
          btnNetplay.textContent = `🌐 Room: ${this.netplay.roomCode}`;
        } else if (state === 'HOSTING') {
          btnNetplay.style.background = '#d35400';
          btnNetplay.style.color = '#fff';
          btnNetplay.textContent = `🌐 Hosting: ${this.netplay.roomCode}`;
        } else {
          btnNetplay.style.background = '#1b4d3e';
          btnNetplay.style.color = '#2ecc71';
          btnNetplay.textContent = '🌐 Netplay (P2P)';
        }
      }
    };

    this.netplay.onPlayerUpdate = (slots, mySlot) => {
      if (hostPlayerList) {
        hostPlayerList.innerHTML = '';
        slots.forEach((s, idx) => {
          const div = document.createElement('div');
          if (s !== null) {
            const isMe = idx === mySlot;
            div.style.color = '#2ecc71';
            div.textContent = `● Slot ${idx + 1}: ${isMe ? 'You' : 'Connected Player'}${idx === 0 ? ' (Host)' : ''}`;
          } else {
            div.style.color = '#7f8c8d';
            div.textContent = `○ Slot ${idx + 1}: Waiting / CPU...`;
          }
          hostPlayerList.appendChild(div);
        });
      }
    };

    this.netplay.onGameStart = (data) => {
      if (modal) modal.style.display = 'none';
      if (!this.exports) return;

      // Seed deterministic generator
      this.exports.mb_seed(data.seed);
      // Peers only share the seed, so every peer must generate the same (procedural) map
      this.exports.mb_reset_map_selection();

      // Configure players vs bots
      data.slots.forEach((slot, idx) => {
        if (slot !== null) {
          this.exports.mb_set_bot_difficulty(idx, 0); // Human
        } else {
          this.exports.mb_set_bot_difficulty(idx, 2); // CPU Medium
        }
      });

      // Start match in WASM: simulate Enter on New Game & Shop
      this.exports.mb_handle_key(KEY_BOMB); // Enter shop
      this.exports.mb_handle_key(KEY_BOMB); // Start arena
      this.audio.startBgm();
      this.renderFramebuffer();
    };

    this.netplay.onFrameSync = (frame, inputs) => {
      if (!this.netplay.isHost && this.exports) {
        this.exports.mb_step(inputs[0], inputs[1], inputs[2], inputs[3]);
        this.renderFramebuffer();
        this.renderNetplayHud();
      }
    };

    this.netplay.onLanRoomsUpdate = (rooms) => {
      this.renderLanRooms(rooms);
    };

    setInterval(() => {
      if (pingDisplay) {
        pingDisplay.textContent = this.netplay.pingMs > 0 ? `Ping: ${this.netplay.pingMs} ms` : 'Ping: -- ms';
      }
    }, 1000);
  }

  renderLanRooms(rooms) {
    const list = document.getElementById('lan-room-list');
    if (!list) return;
    list.innerHTML = '';
    if (rooms.length === 0) {
      list.innerHTML = '<div style="font-size:12px;color:#7f8c8d;text-align:center;padding:20px 0;">No active rooms found on LAN. Host a room or invite friends!</div>';
      return;
    }
    rooms.forEach(r => {
      const row = document.createElement('div');
      row.style.cssText = 'display:flex;justify-content:space-between;align-items:center;background:#1e2433;border:1px solid #3c435c;border-radius:4px;padding:6px 10px;';
      row.innerHTML = `
        <div>
          <span style="color:#f1c40f;font-weight:bold;font-family:monospace;letter-spacing:1px;">[${r.roomCode}]</span>
          <span style="color:#fff;font-size:12px;margin-left:6px;">${r.hostName}</span>
          <span style="color:#95a5a6;font-size:11px;margin-left:6px;">(${r.mode})</span>
        </div>
        <button style="padding:4px 10px;background:#27ae60;border:none;font-weight:bold;color:#fff;border-radius:3px;cursor:pointer;">JOIN (${r.players}/4)</button>
      `;
      row.querySelector('button').addEventListener('click', () => {
        this.netplay.joinRoom(r.roomCode);
        const joinCodeInput = document.getElementById('join-code-input');
        if (joinCodeInput) joinCodeInput.value = r.roomCode;
        const tabJoin = document.getElementById('tab-join');
        const panelJoin = document.getElementById('panel-join');
        [document.getElementById('tab-host'), tabJoin, document.getElementById('tab-lan')].forEach(t => t?.classList.remove('active'));
        [document.getElementById('panel-host'), panelJoin, document.getElementById('panel-lan')].forEach(p => p && (p.style.display = 'none'));
        if (tabJoin) tabJoin.classList.add('active');
        if (panelJoin) panelJoin.style.display = 'block';
      });
      list.appendChild(row);
    });
  }

  renderNetplayHud() {
    this.ctx.fillStyle = 'rgba(0, 0, 0, 0.75)';
    this.ctx.fillRect(470, 4, 166, 18);
    this.ctx.strokeStyle = '#2ecc71';
    this.ctx.lineWidth = 1;
    this.ctx.strokeRect(470, 4, 166, 18);

    this.ctx.font = 'bold 10px monospace';
    this.ctx.fillStyle = '#2ecc71';
    const slot = this.netplay.mySlot + 1;
    const ping = this.netplay.pingMs;
    const text = `🌐 P${slot} [${this.netplay.roomCode || 'LOCAL'}] ${ping}ms`;
    this.ctx.fillText(text, 476, 16);
  }

  loop(timestamp) {
    if (!this.running) return;

    const delta = timestamp - this.lastFrameTime;
    const requiredInterval = this.tickInterval / this.speed;

    if (delta >= requiredInterval) {
      this.lastFrameTime = timestamp - (delta % requiredInterval);

      if (this.exports) {
        const state = this.exports.mb_get_state(); // 0: Title, 1: MainMenu, 2: Options, 3: Info, 4: Shop, 5: Battle, 6: RoundEnd, 7: HallOfFame

        if (state === 5) {
          // Battle simulation frame
          const padInputs = this.gamepad.getPlayerInputs();
          const touchInputs = this.touch.getInputs();
          const localInput = (this.keyState.p1 | padInputs[0] | touchInputs) || 0;

          if (this.netplay.state === 'IN_GAME') {
            const frame = this.netplay.currentFrame++;
            this.netplay.sendLocalInput(frame, localInput);

            if (this.netplay.isHost) {
              const inputs = this.netplay.remoteInputs.get(frame) || [0, 0, 0, 0];
              inputs[0] = localInput;
              this.netplay.broadcastFrameInputs(frame, inputs);
              this.exports.mb_step(inputs[0], inputs[1], inputs[2], inputs[3]);
              this.renderFramebuffer();
              this.renderNetplayHud();
            }
          } else {
            const p1 = localInput;
            const p2 = this.keyState.p2 | padInputs[1];
            const p3 = padInputs[2];
            const p4 = padInputs[3];

            this.exports.mb_step(p1, p2, p3, p4);
            this.renderFramebuffer();
          }
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

        // Process rumble events from WASM (gamepad & mobile haptics)
        while (this.exports.mb_get_rumble_event(this.rumblePtr) === 1) {
          const pIdx = Math.round(memF32[rumbleOffset]);
          const intensity = memF32[rumbleOffset + 1];
          const duration = memF32[rumbleOffset + 2];
          this.gamepad.rumble(pIdx, intensity, duration);
          if (pIdx === 0 && intensity > 0.2) {
            this.touch.vibrate(Math.min(120, Math.max(20, Math.round(duration * 0.8))));
          }
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
