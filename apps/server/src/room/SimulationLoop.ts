import {
  CHECKPOINT_INTERVAL_MS,
  SIM_STEP_MS,
  type GameEvent,
  type WorldSimulation,
} from '@minebombers/shared';

export const SNAPSHOT_INTERVAL_MS = 100;

export interface SimulationLoopCallbacks {
  onEvents(events: GameEvent[]): void;
  onSnapshot(): void;
  onCheckpoint(): Promise<void>;
  onRoundEnded(reason: string): void;
}

export class SimulationLoop {
  private loopTimer: ReturnType<typeof setTimeout> | null = null;
  private lastWallTimeMs = 0;
  private accumulatorMs = 0;
  private lastSnapshotAtMs = 0;
  private lastCheckpointAtMs = 0;
  private isRunning = false;

  constructor(
    private readonly sim: WorldSimulation,
    private readonly callbacks: SimulationLoopCallbacks,
  ) {}

  public start(): void {
    if (this.isRunning) return;
    this.isRunning = true;
    this.lastWallTimeMs = Date.now();
    this.accumulatorMs = 0;
    this.lastSnapshotAtMs = Date.now();
    this.lastCheckpointAtMs = Date.now();

    const run = async () => {
      if (!this.isRunning || this.sim.phase !== 'playing') {
        this.stop();
        return;
      }

      const now = Date.now();
      const elapsed = Math.min(now - this.lastWallTimeMs, 250);
      this.lastWallTimeMs = now;
      this.accumulatorMs += elapsed;

      let steps = 0;
      while (this.accumulatorMs >= SIM_STEP_MS && steps < 5) {
        const events = this.sim.step(SIM_STEP_MS);
        if (events.length > 0) {
          this.callbacks.onEvents(events);
        }
        this.accumulatorMs -= SIM_STEP_MS;
        steps++;

        if (this.sim.phase !== 'playing') {
          this.callbacks.onRoundEnded('condition_met');
          this.stop();
          return;
        }
      }

      if (now - this.lastSnapshotAtMs >= SNAPSHOT_INTERVAL_MS) {
        this.callbacks.onSnapshot();
        this.lastSnapshotAtMs = now;
      }

      if (now - this.lastCheckpointAtMs >= CHECKPOINT_INTERVAL_MS) {
        await this.callbacks.onCheckpoint();
        this.lastCheckpointAtMs = now;
      }

      if (this.isRunning) {
        this.loopTimer = setTimeout(run, Math.max(0, SIM_STEP_MS - 2));
      }
    };

    this.loopTimer = setTimeout(run, SIM_STEP_MS);
  }

  public stop(): void {
    this.isRunning = false;
    if (this.loopTimer) {
      clearTimeout(this.loopTimer);
      this.loopTimer = null;
    }
  }
}
