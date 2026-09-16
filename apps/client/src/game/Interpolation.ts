export interface SnapshotPosition {
  time: number;
  x: number;
  y: number;
}

export class RemoteEntityInterpolation {
  private buffer: SnapshotPosition[] = [];
  private readonly bufferTimeMs = 100; // 100ms interpolation delay

  public pushSnapshot(x: number, y: number, time = Date.now()): void {
    this.buffer.push({ x, y, time });
    // Keep max 20 snapshots
    if (this.buffer.length > 20) {
      this.buffer.shift();
    }
  }

  public getInterpolatedPosition(renderTime = Date.now() - this.bufferTimeMs): { x: number; y: number } {
    if (this.buffer.length === 0) return { x: 0, y: 0 };
    if (this.buffer.length === 1) return { x: this.buffer[0]!.x, y: this.buffer[0]!.y };

    // If renderTime is before the earliest buffer item
    if (renderTime <= this.buffer[0]!.time) {
      return { x: this.buffer[0]!.x, y: this.buffer[0]!.y };
    }

    // If renderTime is after the newest buffer item
    const latest = this.buffer[this.buffer.length - 1]!;
    if (renderTime >= latest.time) {
      return { x: latest.x, y: latest.y };
    }

    // Find two snapshots surrounding renderTime
    for (let i = 0; i < this.buffer.length - 1; i++) {
      const p0 = this.buffer[i]!;
      const p1 = this.buffer[i + 1]!;

      if (renderTime >= p0.time && renderTime <= p1.time) {
        const span = p1.time - p0.time;
        const progress = span > 0 ? (renderTime - p0.time) / span : 0;
        return {
          x: p0.x + (p1.x - p0.x) * progress,
          y: p0.y + (p1.y - p0.y) * progress,
        };
      }
    }

    return { x: latest.x, y: latest.y };
  }
}
