import Phaser from 'phaser';

export interface DebugMetrics {
  fps: number;
  rtt: number;
  serverSeq: number;
  predictionErrorPx: number;
  entitiesCount: number;
  phase: string;
}

export class DebugOverlay {
  private container!: Phaser.GameObjects.Container;
  private bg!: Phaser.GameObjects.Rectangle;
  private text!: Phaser.GameObjects.Text;
  public visible = false;

  constructor(scene: Phaser.Scene) {
    this.bg = scene.add.rectangle(10, 10, 240, 140, 0x000000, 0.75).setOrigin(0, 0);
    this.text = scene.add.text(20, 20, '', {
      fontSize: '13px',
      fontFamily: 'monospace',
      color: '#00ffcc',
      lineSpacing: 4,
    });

    this.container = scene.add.container(0, 0, [this.bg, this.text]);
    this.container.setDepth(1000);
    this.container.setScrollFactor(0);
    this.container.setVisible(this.visible);

    // Toggle on F3
    scene.input.keyboard?.on('keydown-F3', () => {
      this.toggle();
    });
  }

  public toggle(): void {
    this.visible = !this.visible;
    this.container.setVisible(this.visible);
  }

  public update(metrics: DebugMetrics): void {
    if (!this.visible) return;
    this.text.setText([
      `FPS: ${metrics.fps.toFixed(0)}`,
      `RTT: ${metrics.rtt} ms`,
      `Seq: ${metrics.serverSeq}`,
      `Phase: ${metrics.phase}`,
      `Pred Err: ${metrics.predictionErrorPx.toFixed(1)} px`,
      `Entities: ${metrics.entitiesCount}`,
      `[F3] to toggle debug`,
    ]);
  }

  public destroy(): void {
    this.container.destroy();
  }
}
