import { movePlayerWithCollision, type SimTileState } from '@minebombers/shared';

export interface UnackedInput {
  seq: number;
  dx: -1 | 0 | 1;
  dy: -1 | 0 | 1;
  stepMs: number;
}

export class ClientPrediction {
  public predictedX = 0;
  public predictedY = 0;
  private unackedInputs: UnackedInput[] = [];

  public init(x: number, y: number): void {
    this.predictedX = x;
    this.predictedY = y;
    this.unackedInputs = [];
  }

  public predictMove(
    dx: -1 | 0 | 1,
    dy: -1 | 0 | 1,
    stepMs: number,
    seq: number,
    tiles: readonly SimTileState[],
  ): void {
    this.unackedInputs.push({ seq, dx, dy, stepMs });
    const res = movePlayerWithCollision(this.predictedX, this.predictedY, dx, dy, stepMs, tiles);
    this.predictedX = res.x;
    this.predictedY = res.y;
  }

  public reconcile(
    authoritativeX: number,
    authoritativeY: number,
    ackInputSeq: number,
    tiles: readonly SimTileState[],
  ): { errorPx: number; correctedX: number; correctedY: number } {
    // Discard acknowledged inputs
    this.unackedInputs = this.unackedInputs.filter((inp) => inp.seq > ackInputSeq);

    // Replay unacked inputs from authoritative base
    let replayedX = authoritativeX;
    let replayedY = authoritativeY;

    for (const inp of this.unackedInputs) {
      const res = movePlayerWithCollision(replayedX, replayedY, inp.dx, inp.dy, inp.stepMs, tiles);
      replayedX = res.x;
      replayedY = res.y;
    }

    const errorUnits = Math.hypot(this.predictedX - replayedX, this.predictedY - replayedY);
    const errorPx = errorUnits / 32; // 1024 units = 32 px -> 32 units / px

    this.predictedX = replayedX;
    this.predictedY = replayedY;

    return {
      errorPx,
      correctedX: replayedX,
      correctedY: replayedY,
    };
  }
}
