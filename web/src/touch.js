// Mobile & Tablet Virtual Touch Controller for Mine Bombers

export class TouchController {
  constructor(game) {
    this.game = game;
    this.enabled = false;
    this.container = document.getElementById('touch-controls-container');
    this.btnToggle = document.getElementById('btn-touch');

    // Current battle bitmask: Up(1), Down(2), Left(4), Right(8), Bomb(16), Choose(32), Remote(64)
    this.battleMask = 0;

    // Active touch tracking
    this.activeTouches = new Map(); // touchId -> actionName ('up', 'down', 'left', 'right', 'bomb', 'choose', 'remote', 'tab', 'esc')

    // Menu debounce timers
    this.menuRepeatTimers = new Map();

    this.init();
  }

  init() {
    if (!this.container) return;

    // Auto-detect touch device
    const isTouchDevice = ('ontouchstart' in window) ||
      (navigator.maxTouchPoints > 0) ||
      (navigator.msMaxTouchPoints > 0) ||
      /Android|iPhone|iPad|iPod|Windows Phone/i.test(navigator.userAgent);

    if (isTouchDevice) {
      this.setEnabled(true);
    }

    if (this.btnToggle) {
      this.btnToggle.addEventListener('click', () => {
        this.setEnabled(!this.enabled);
      });
    }

    this.bindTouchEvents();
  }

  setEnabled(enabled) {
    this.enabled = enabled;
    if (this.container) {
      this.container.style.display = enabled ? 'flex' : 'none';
    }
    if (this.btnToggle) {
      this.btnToggle.classList.toggle('active', enabled);
    }
  }

  vibrate(ms = 15) {
    if (typeof navigator.vibrate === 'function') {
      try {
        navigator.vibrate(ms);
      } catch (_) {}
    }
  }

  bindTouchEvents() {
    // Action definitions mapped to DOM IDs and WASM keys
    const buttonConfigs = [
      { id: 'touch-up', action: 'up', wasmKey: 1, maskBit: 1 },
      { id: 'touch-down', action: 'down', wasmKey: 2, maskBit: 2 },
      { id: 'touch-left', action: 'left', wasmKey: 3, maskBit: 4 },
      { id: 'touch-right', action: 'right', wasmKey: 4, maskBit: 8 },
      { id: 'touch-btn-bomb', action: 'bomb', wasmKey: 5, maskBit: 16 },
      { id: 'touch-btn-choose', action: 'choose', wasmKey: 6, maskBit: 32 },
      { id: 'touch-btn-remote', action: 'remote', wasmKey: 7, maskBit: 64 },
      { id: 'touch-btn-tab', action: 'tab', wasmKey: 8, maskBit: 0 },
      { id: 'touch-btn-esc', action: 'esc', wasmKey: 9, maskBit: 0 },
    ];

    const actionMap = new Map();
    buttonConfigs.forEach(cfg => {
      const el = document.getElementById(cfg.id);
      if (el) {
        actionMap.set(cfg.action, { ...cfg, el });
      }
    });

    const handlePress = (action) => {
      const item = actionMap.get(action);
      if (!item) return;

      this.vibrate(15);
      item.el.classList.add('pressed');

      if (this.game && this.game.exports) {
        const state = this.game.exports.mb_get_state();
        if (state === 5) {
          // Battle mode: update bitmask
          if (item.maskBit) {
            this.battleMask |= item.maskBit;
          }
        } else {
          // Menu / Shop mode: discrete key event
          if (item.wasmKey) {
            const newState = this.game.exports.mb_handle_key(item.wasmKey);
            if (newState === 5 && this.game.audio) {
              this.game.audio.startBgm();
            }
            this.game.renderFramebuffer();

            // Set up auto-repeat for menu navigation if held
            if (['up', 'down', 'left', 'right'].includes(action)) {
              this.clearMenuRepeat(action);
              const timer = setInterval(() => {
                if (this.game.exports && this.game.exports.mb_get_state() !== 5) {
                  this.game.exports.mb_handle_key(item.wasmKey);
                  this.game.renderFramebuffer();
                }
              }, 160);
              this.menuRepeatTimers.set(action, timer);
            }
          }
        }
      }
    };

    const handleRelease = (action) => {
      const item = actionMap.get(action);
      if (!item) return;

      item.el.classList.remove('pressed');
      this.clearMenuRepeat(action);

      if (item.maskBit) {
        this.battleMask &= ~item.maskBit;
      }
    };

    // Bind individual touch buttons
    actionMap.forEach((item, action) => {
      const el = item.el;

      el.addEventListener('touchstart', (e) => {
        e.preventDefault();
        e.stopPropagation();
        if (this.game && this.game.audio) {
          this.game.audio.ensureContext();
        }
        for (let i = 0; i < e.changedTouches.length; i++) {
          const t = e.changedTouches[i];
          this.activeTouches.set(t.identifier, action);
        }
        handlePress(action);
      }, { passive: false });

      el.addEventListener('touchend', (e) => {
        e.preventDefault();
        e.stopPropagation();
        for (let i = 0; i < e.changedTouches.length; i++) {
          const t = e.changedTouches[i];
          this.activeTouches.delete(t.identifier);
        }
        handleRelease(action);
      }, { passive: false });

      el.addEventListener('touchcancel', (e) => {
        e.preventDefault();
        e.stopPropagation();
        for (let i = 0; i < e.changedTouches.length; i++) {
          const t = e.changedTouches[i];
          this.activeTouches.delete(t.identifier);
        }
        handleRelease(action);
      }, { passive: false });
    });

    // Support smooth sliding over D-Pad area
    const dpadArea = document.getElementById('touch-dpad');
    if (dpadArea) {
      dpadArea.addEventListener('touchmove', (e) => {
        e.preventDefault();
        e.stopPropagation();

        for (let i = 0; i < e.changedTouches.length; i++) {
          const touch = e.changedTouches[i];
          const targetEl = document.elementFromPoint(touch.clientX, touch.clientY);
          const oldAction = this.activeTouches.get(touch.identifier);

          let newAction = null;
          if (targetEl) {
            const btn = targetEl.closest('.touch-dpad-btn');
            if (btn) {
              const cfg = buttonConfigs.find(c => c.id === btn.id);
              if (cfg) newAction = cfg.action;
            }
          }

          if (oldAction !== newAction) {
            if (oldAction) handleRelease(oldAction);
            if (newAction) {
              this.activeTouches.set(touch.identifier, newAction);
              handlePress(newAction);
            } else {
              this.activeTouches.delete(touch.identifier);
            }
          }
        }
      }, { passive: false });
    }
  }

  clearMenuRepeat(action) {
    if (this.menuRepeatTimers.has(action)) {
      clearInterval(this.menuRepeatTimers.get(action));
      this.menuRepeatTimers.delete(action);
    }
  }

  getInputs() {
    return this.battleMask;
  }
}
