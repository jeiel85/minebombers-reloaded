# 04. Authoritative Simulation and Client Prediction

## Rule

**The server decides truth; the client decides presentation.**

The browser never authoritatively sets position, dig completion, cash, inventory, HP, explosion result or match result.

## Fixed-step simulation

```text
SIM_STEP_MS = 50       # 20 Hz logical world
snapshot max = 10 Hz
```

During an active round the Durable Object runs a server-owned timer loop. Do not make bomb fuses or the round clock depend on receiving client movement messages.

See `15-server-timing-cost.md` for the lifecycle/cost model.

## World update order per logical step

Use this deterministic order:

1. expire/neutralize invalid inputs;
2. apply player movement intent;
3. resolve world/player collision;
4. accumulate and resolve digging;
5. process discrete actions queued for this step;
6. move projectiles if later added;
7. resolve mine triggers;
8. resolve due explosives in stable ID order;
9. resolve chain explosions;
10. apply damage/death once per source;
11. resolve pickups/treasure;
12. evaluate round-end conditions;
13. increment simulation sequence/time.

Do not rely on JavaScript object/map incidental iteration order for gameplay where an explicit sort key is available.

## Movement

Client sends direction intent, never coordinates.

```text
1 tile = 1024 world units
base speed = 3000 units/sec
player AABB = 576 x 576 units
```

Diagonal input is normalized.

## Input freshness

Input-on-change is allowed because the server owns the loop. Each held movement state must be refreshed every 500 ms. If no refresh is received for 1000 ms, neutralize movement as a safety rule even if the socket has not closed.

## Local prediction

Client:

1. increments `inputSeq` on state changes;
2. applies movement locally immediately;
3. stores unacknowledged input states;
4. receives server snapshot with `ackInputSeq`;
5. resets predicted local position to authoritative position;
6. replays newer input history;
7. visually smooths small corrections.

Recommended correction policy:

```text
<= 4 px        soft/no visible correction
4..32 px       blend over ~100 ms
> 32 px        hard correction
```

Tune by playtest.

## Remote interpolation

Render remote players roughly 100–150 ms behind estimated server time and interpolate between snapshots. Do not extrapolate authoritative damage/death as truth.

## Dig prediction

Client may show immediate pickaxe animation and estimated progress while server confirmation is pending. A soil tile becomes visually traversable only when authoritative tile change arrives or when a predicted change is immediately reconciled by the server.

## Explosive timing

When placement is accepted the server creates an absolute simulation deadline. Client animates the fuse from the deadline; only server event/state decides the blast cells, damage and destruction.

## Recovery

Active world checkpoint every ~2 seconds. On Durable Object restart, restore checkpoint, reconcile deadlines to current time, then send a full state resync before accepting normal play. See `16-storage-reconnect-contract.md`.
