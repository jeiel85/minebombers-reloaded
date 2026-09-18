export class GamepadManager {
  constructor() {
    this.deadzone = 0.35;
  }

  getPlayerInputs() {
    const gamepads = typeof navigator.getGamepads === 'function' ? navigator.getGamepads() : [];
    const inputs = [0, 0, 0, 0];

    for (let i = 0; i < 4; i++) {
      const pad = gamepads[i];
      if (!pad) continue;

      let mask = 0;
      const axX = pad.axes[0] || 0;
      const axY = pad.axes[1] || 0;

      // D-pad & Sticks
      const up = pad.buttons[12]?.pressed || axY < -this.deadzone;
      const down = pad.buttons[13]?.pressed || axY > this.deadzone;
      const left = pad.buttons[14]?.pressed || axX < -this.deadzone;
      const right = pad.buttons[15]?.pressed || axX > this.deadzone;

      // Action buttons
      // A (0): Bomb
      const bomb = pad.buttons[0]?.pressed;
      // X (2): Choose weapon
      const choose = pad.buttons[2]?.pressed;
      // Y (3) or RB (5): Remote
      const remote = pad.buttons[3]?.pressed || pad.buttons[5]?.pressed;
      // B (1) or LB (4): Stop
      const stop = pad.buttons[1]?.pressed || pad.buttons[4]?.pressed;

      if (up) mask |= 1;
      if (down) mask |= 2;
      if (left) mask |= 4;
      if (right) mask |= 8;
      if (bomb) mask |= 16;
      if (choose) mask |= 32;
      if (remote) mask |= 64;
      if (stop) mask |= 128;

      inputs[i] = mask;
    }

    return inputs;
  }

  rumble(playerIdx, intensity, durationMs) {
    const gamepads = typeof navigator.getGamepads === 'function' ? navigator.getGamepads() : [];
    const pad = gamepads[playerIdx];
    if (!pad) return;

    if (pad.vibrationActuator && typeof pad.vibrationActuator.playEffect === 'function') {
      pad.vibrationActuator.playEffect('dual-rumble', {
        startDelay: 0,
        duration: Math.max(50, Math.min(1000, durationMs)),
        weakMagnitude: Math.max(0.0, Math.min(1.0, intensity * 0.8)),
        strongMagnitude: Math.max(0.0, Math.min(1.0, intensity)),
      }).catch(() => {});
    }
  }

  rumbleAll(intensity, durationMs) {
    for (let i = 0; i < 4; i++) {
      this.rumble(i, intensity, durationMs);
    }
  }
}
