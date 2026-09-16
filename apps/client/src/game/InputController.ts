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
  private key1!: Phaser.Input.Keyboard.Key;
  private key2!: Phaser.Input.Keyboard.Key;
  private key3!: Phaser.Input.Keyboard.Key;
  private key4!: Phaser.Input.Keyboard.Key;

  private lastState: InputState = { dx: 0, dy: 0, primary: false, secondary: false, slot: 0 };
  private lastTransmittedAt = 0;
  private selectedSlot = 0;

  constructor(scene: Phaser.Scene) {
    if (scene.input.keyboard) {
      this.cursors = scene.input.keyboard.createCursorKeys();
      this.keyW = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.W);
      this.keyA = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.A);
      this.keyS = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.S);
      this.keyD = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.D);
      this.keySpace = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.SPACE);
      this.keyE = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.E);
      this.key1 = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.ONE);
      this.key2 = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.TWO);
      this.key3 = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.THREE);
      this.key4 = scene.input.keyboard.addKey(Phaser.Input.Keyboard.KeyCodes.FOUR);
    }
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

    if (this.key1?.isDown) this.selectedSlot = 0;
    if (this.key2?.isDown) this.selectedSlot = 1;
    if (this.key3?.isDown) this.selectedSlot = 2;
    if (this.key4?.isDown) this.selectedSlot = 3;

    const primary = Phaser.Input.Keyboard.JustDown(this.keySpace) || false;
    const secondary = Phaser.Input.Keyboard.JustDown(this.keyE) || false;

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
