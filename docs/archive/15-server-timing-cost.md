# 15. Server Timing Model and Serverless Cost Behavior

## Critical design decision

Use two lifecycle modes for each Durable Object room.

### Lobby / shop / result idle

Use the Hibernation WebSocket API and **no repeating JavaScript timer**. The object may hibernate while sockets remain connected.

### Active round

Run an authoritative fixed-step simulation driver while the round is active. This intentionally keeps the object non-hibernatable for the duration of the round. Correct bomb fuses, mine triggers, round timer and snapshots must not depend on a player continuing to send movement packets.

Cloudflare documentation explicitly notes that scheduled callbacks such as `setTimeout`/`setInterval` prevent hibernation. That is acceptable during active gameplay and should be stopped immediately when the round ends.

## Simulation driver

Logical simulation:

```text
SIM_STEP_MS = 50              # 20 Hz authoritative physics/game rules
SNAPSHOT_INTERVAL_MS = 100    # max 10 Hz network snapshots
CHECKPOINT_INTERVAL_MS = 2000 # recoverable checkpoint, not every tick
```

Recommended class fields:

```ts
private loopTimer: ReturnType<typeof setTimeout> | null = null;
private lastWallTimeMs = 0;
private accumulatorMs = 0;
private lastSnapshotAtMs = 0;
private lastCheckpointAtMs = 0;
```

Use a one-shot `setTimeout` that schedules the next iteration rather than uncontrolled `setInterval`, so overruns can be measured and drift/catch-up can be capped.

Pseudo-code:

```ts
startRoundLoop() {
  if (this.loopTimer) return;
  this.lastWallTimeMs = Date.now();

  const run = async () => {
    if (this.room.phase !== 'playing') {
      this.loopTimer = null;
      return;
    }

    const now = Date.now();
    const elapsed = Math.min(now - this.lastWallTimeMs, 250);
    this.lastWallTimeMs = now;
    this.accumulatorMs += elapsed;

    let steps = 0;
    while (this.accumulatorMs >= SIM_STEP_MS && steps < 5) {
      stepWorld(SIM_STEP_MS);
      this.accumulatorMs -= SIM_STEP_MS;
      steps++;
    }

    if (now - this.lastSnapshotAtMs >= SNAPSHOT_INTERVAL_MS) {
      broadcastSnapshot();
      this.lastSnapshotAtMs = now;
    }

    if (now - this.lastCheckpointAtMs >= CHECKPOINT_INTERVAL_MS) {
      await saveCheckpoint();
      this.lastCheckpointAtMs = now;
    }

    this.loopTimer = setTimeout(run, Math.max(0, SIM_STEP_MS - 2));
  };

  this.loopTimer = setTimeout(run, SIM_STEP_MS);
}
```

Production code must handle exceptions and guarantee the loop is stopped/cleared when a round ends.

## Why not 50 ms alarms

Durable Object Alarms are for scheduled future work and are billed as invocations/writes. Cloudflare recommends avoiding short recurring alarms when there is frequent work. Do not implement the 20 Hz game loop as a chain of storage alarms.

Use alarms for coarse lifecycle work only, such as shop/result phase deadlines, empty-room expiry or a failsafe deadline. A room has only one Durable Object alarm, so store all pending coarse deadlines and schedule the alarm for the nearest one. When `alarm()` runs, process every due deadline and then schedule the next nearest deadline.

Examples:

```text
shop close deadline      -> alarm at closesAt
countdown -> playing     -> alarm at startsAt if no active loop yet
round result -> shop     -> alarm at resultEndsAt
empty-room expiry        -> alarm at expiresAt when it is the nearest deadline
```

Do not use alarms as the 20 Hz simulation clock.

## Input transmission

Because the server owns the active loop, clients do not need to send unchanged input at 10 Hz forever.

v1 policy:

```text
input state changed: send immediately
while held unchanged: refresh every 500 ms
ping/liveness: every 5 s
combat actions: immediate discrete messages
```

The refresh prevents a lost/disconnected state from leaving stale movement indefinitely and gives observability. The server also neutralizes input immediately on WebSocket close.

## Snapshot behavior

Outgoing WebSocket messages are not request-billed under current Cloudflare pricing, but still contribute to execution time while code runs. Broadcast at <= 10 Hz for v1 and profile before increasing it.

## Free-tier duration sanity check

Current Free plan documentation lists 13,000 GB-s/day for Durable Object duration. A Durable Object is metered as 128 MB (= 0.125 GB) while active/non-hibernatable.

Approximate theoretical active-room wall time from duration alone:

```text
13,000 GB-s / 0.125 GB = 104,000 seconds
104,000 / 3600 = 28.9 active room-hours/day
```

This is an upper-bound planning figure, not a service guarantee. Request limits, storage operations and other usage also apply.

With input-on-change rather than 10 input messages/sec/player, request usage can be much lower than the first revision's conservative model.

## Failure recovery

Every 2 seconds persist a compact recoverable checkpoint containing:

- phase/match/round IDs;
- simulation time;
- seed/generator version;
- players and authoritative economy/inventory;
- changed tiles;
- active explosives and their absolute deadlines;
- treasure entities;
- round deadline.

If the Durable Object restarts during `playing`, load the checkpoint, advance scheduled consequences to current time with a capped recovery routine, broadcast a full resync and restart the active loop.
