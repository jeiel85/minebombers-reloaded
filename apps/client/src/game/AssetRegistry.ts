import Phaser from 'phaser';

export class AssetRegistry {
  public static preload(scene: Phaser.Scene): void {
    scene.load.spritesheet('tiles_world', './assets/gfx/tiles_world.png', {
      frameWidth: 32,
      frameHeight: 32,
    });
    scene.load.spritesheet('miner', './assets/gfx/miner.png', {
      frameWidth: 32,
      frameHeight: 32,
    });
    scene.load.spritesheet('bombs', './assets/gfx/bombs.png', {
      frameWidth: 32,
      frameHeight: 32,
    });
    scene.load.spritesheet('pickups', './assets/gfx/pickups.png', {
      frameWidth: 32,
      frameHeight: 32,
    });
    scene.load.spritesheet('explosions', './assets/gfx/explosions.png', {
      frameWidth: 32,
      frameHeight: 32,
    });
    scene.load.spritesheet('ui_icons', './assets/gfx/ui_icons.png', {
      frameWidth: 32,
      frameHeight: 32,
    });
    scene.load.spritesheet('monsters', './assets/gfx/monsters.png', {
      frameWidth: 32,
      frameHeight: 32,
    });
    scene.load.spritesheet('projectiles', './assets/gfx/projectiles.png', {
      frameWidth: 32,
      frameHeight: 32,
    });
  }

  public static createAnimations(scene: Phaser.Scene): void {
    if (!scene.anims.exists('miner_down')) {
      scene.anims.create({
        key: 'miner_down',
        frames: scene.anims.generateFrameNumbers('miner', { start: 0, end: 3 }),
        frameRate: 8,
        repeat: -1,
      });
      scene.anims.create({
        key: 'miner_left',
        frames: scene.anims.generateFrameNumbers('miner', { start: 4, end: 7 }),
        frameRate: 8,
        repeat: -1,
      });
      scene.anims.create({
        key: 'miner_right',
        frames: scene.anims.generateFrameNumbers('miner', { start: 8, end: 11 }),
        frameRate: 8,
        repeat: -1,
      });
      scene.anims.create({
        key: 'miner_up',
        frames: scene.anims.generateFrameNumbers('miner', { start: 12, end: 15 }),
        frameRate: 8,
        repeat: -1,
      });
      scene.anims.create({
        key: 'bomb_tick',
        frames: scene.anims.generateFrameNumbers('bombs', { start: 0, end: 3 }),
        frameRate: 4,
        repeat: -1,
      });
      scene.anims.create({
        key: 'explode',
        frames: scene.anims.generateFrameNumbers('explosions', { start: 0, end: 4 }),
        frameRate: 15,
        repeat: 0,
      });
      scene.anims.create({
        key: 'slime_idle',
        frames: scene.anims.generateFrameNumbers('monsters', { start: 0, end: 1 }),
        frameRate: 3,
        repeat: -1,
      });
      scene.anims.create({
        key: 'bat_fly',
        frames: scene.anims.generateFrameNumbers('monsters', { start: 2, end: 3 }),
        frameRate: 6,
        repeat: -1,
      });
    }
  }

  public static readonly PLAYER_COLORS = [
    0x3498db, // 1: Blue
    0xe74c3c, // 2: Red
    0x2ecc71, // 3: Green
    0xf1c40f, // 4: Yellow
    0x9b59b6, // 5: Purple
    0xe67e22, // 6: Orange
    0x1abc9c, // 7: Teal
    0xff79c6, // 8: Pink
  ];

  public static getPlayerColor(index: number): number {
    return this.PLAYER_COLORS[index % this.PLAYER_COLORS.length] ?? 0xffffff;
  }
}
