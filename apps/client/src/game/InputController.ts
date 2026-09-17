import Phaser from 'phaser';
import { HELD_INPUT_REFRESH_MS } from '@minebombers/shared';

export interface InputState {
  dx: -1 | 0 | 1;
  dy: -1 | 0 | 1;
  primary: boolean;
  secondary: boolean;
  slot: number;
}

export class InputController {
  private cursors!: Phaser.Types.Input.Keyboard.CursorKeys;
  private keyW!: Phaser.Input.Keyboard.Key;
  private keyA!: Phaser.Input.Keyboard.Key;
  private keyS!: Phaser.Input.Keyboard.Key;
  private keyD!: Phaser.Input.Keyboard.Key;
  private keySpace!: Phaser.Input.Keyboard.Key;
  private keyE!: Phaser.Input.Keyboard.Key;
  private keyQ!: Phaser.Input.Keyboard.Key;
  private keyF!: Phaser.Input.Keyboard.Key;
  private keyC!: Phaser.Input.Keyboard.Key;
  private keyShift!: Phaser.Input.Keyboard.Key;
  private numberKeys: Phaser.Input.Keyboard.Key[] = [];

  private lastState: InputState = { dx: 0, dy: 0, primary: false, secondary: false, slot: 0 };
  private lastTransmittedAt = 0;
  private selectedSlot = 0;
  private rightClickTriggered = false;

  constructor(scene: Phaser.Scene) {
    if (scene.input.keyboard) {
      this.cursors = scene.input.keyboard.createCursorKeys();
      this.keyW = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.W);
      this.keyA = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.A);
      this.keyS = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.S);
      this.keyD = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.D);
      this.keySpace = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.SPACE);
      this.keyQ = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.Q);
      this.keyE = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.E);
      this.keyF = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.F);
      this.keyC = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.C);
      this.keyShift = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.SHIFT);

      const numCodes = [
        Phaser.Input.Keyboard.KeyCodes.ONE,
        Phaser.Input.Keyboard.KeyCodes.TWO,
        Phaser.Input.Keyboard.KeyCodes.THREE,
        Phaser.Input.Keyboard.KeyCodes.FOUR,
        Phaser.Input.Keyboard.KeyCodes.FIVE,
        Phaser.Input.Keyboard.KeyCodes.SIX,
        Phaser.Input.Keyboard.KeyCodes.SEVEN,
        Phaser.Input.Keyboard.KeyCodes.EIGHT,
      ];
      this.numberKeys = numCodes.map((code) => scene.input.keyboard!.addKey(code));
    }

    // Disable browser context menu on canvas for right-click support
    scene.input.mouse?.disableContextMenu();
    scene.input.on('pointerdown', (pointer: Phaser.Input.Pointer) => {
      if (pointer.rightButtonDown()) {
        this.rightClickTriggered = true;
      }
    });
  }

  public setSlot(slotIndex: number): void {
    if (slotIndex >= 0 && slotIndex < 8) {
      this.selectedSlot = slotIndex;
    }
  }

  public getSelectedSlot(): number {
    return this.selectedSlot;
  }

  public update(): { state: InputState; shouldTransmit: boolean } {
    let dx: -1 | 0 | 1 = 0;
    let dy: -1 | 0 | 1 = 0;

    const left = this.cursors?.left?.isDown || this.keyA?.isDown;
    const right = this.cursors?.right?.isDown || this.keyD?.isDown;
    const up = this.cursors?.up?.isDown || this.keyW?.isDown;
    const down = this.cursors?.down?.isDown || this.keyS?.isDown;

    if (left && !right) dx = -1;
    else if (right && !left) dx = 1;

    if (up && !down) dy = -1;
    else if (down && !up) dy = 1;

    // Number keys 1..8
    for (let i = 0; i < this.numberKeys.length; i++) {
      const k = this.numberKeys[i];
      if (k && Phaser.Input.Keyboard.JustDown(k)) {
        this.selectedSlot = i;
        break;
      }
    }

    // Q (cycle prev) / E (cycle next)
    if (this.keyQ && Phaser.Input.Keyboard.JustDown(this.keyQ)) {
      this.selectedSlot = (this.selectedSlot - 1 + 8) % 8;
    }
    if (this.keyE && Phaser.Input.Keyboard.JustDown(this.keyE)) {
      this.selectedSlot = (this.selectedSlot + 1) % 8;
    }

    const primary = Phaser.Input.Keyboard.JustDown(this.keySpace) || false;
    const secondaryKey =
      (this.keyF && Phaser.Input.Keyboard.JustDown(this.keyF)) ||
      (this.keyC && Phaser.Input.Keyboard.JustDown(this.keyC)) ||
      (this.keyShift && Phaser.Input.Keyboard.JustDown(this.keyShift)) ||
      false;

    const secondary = secondaryKey || this.rightClickTriggered;
    this.rightClickTriggered = false;

    const currentState: InputState = {
      dx,
      dy,
      primary,
      secondary,
      slot: this.selectedSlot,
    };

    const now = Date.now();
    const changed =
      currentState.dx !== this.lastState.dx ||
      currentState.dy !== this.lastState.dy ||
      currentState.primary ||
      currentState.secondary ||
      currentState.slot !== this.lastState.slot;

    const heldRefreshDue = now - this.lastTransmittedAt >= HELD_INPUT_REFRESH_MS;
    const shouldTransmit = changed || heldRefreshDue;

    if (shouldTransmit) {
      this.lastState = currentState;
      this.lastTransmittedAt = now;
    }

    return { state: currentState, shouldTransmit };
  }
}
