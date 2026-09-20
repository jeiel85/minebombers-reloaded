# 19. Classic Mine v1 — Deterministic Map Generator

Generator ID:

```text
classic-mine-v1
```

The exact algorithm is versioned. Changing any rule that changes generated output requires a new generator ID so old recorded matches/checkpoints remain interpretable.

## PRNG

Use a 32-bit integer PRNG with identical implementation on server and client. `mulberry32` is sufficient for map generation v1 because this is deterministic gameplay generation, not cryptography.

```ts
export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
```

Never use this PRNG for resume tokens or security-sensitive randomness.

## Dimensions

```text
width:  31
height: 23
```

Tile index:

```text
index = y * width + x
```

## Spawn candidates

Assign in this order after a deterministic shuffle of player IDs, or by server slot order if stable spawn identity is desired:

```text
(2, 2)
(28, 20)
(28, 2)
(2, 20)
(15, 2)
(15, 20)
(2, 11)
(28, 11)
```

The server sends spawn assignment in the initial authoritative state.

## Generation steps

### 1. Initialize

- entire map = `soil`;
- outermost x=0/x=30/y=0/y=22 = `rock`.

### 2. Carve spawn safe zones

For every active spawn, make all cells within Chebyshev distance <= 1 `floor` (3x3 square), excluding border.

Also carve one extra floor cell toward map center from each spawn so the player cannot be visually boxed in at start.

### 3. Carve central contest area

Make the 3x3 square centered at `(15,11)` floor.

### 4. Place hard-rock pillars

Eligible cell:

- currently soil;
- Manhattan distance >= 4 from every spawn;
- not in central 5x5 region;
- none of its four orthogonal neighbors is already a generated interior rock pillar.

Target count:

```text
floor((width - 2) * (height - 2) * 0.08)
```

Create a list of eligible interior coordinates, Fisher-Yates shuffle using the seeded PRNG, then scan and place rock until target is reached or candidates exhaust. Re-check adjacency at placement time.

This creates isolated blockers without generating solid orthogonal rock walls that can partition the map.

### 5. Treasure selection

Eligible treasure cell:

- tile remains `soil`;
- Manhattan distance >= 3 from every spawn.

Counts:

```text
basic = 18 + 2 * playerCount
rare  = max(2, playerCount)
```

Shuffle eligible indices using the same PRNG stream after rock placement. Consume the first `rare` for rare treasures, then next `basic` for basic treasures.

A treasure remains hidden/attached to its soil tile until that tile is opened by digging/explosion, then spawns as a collectible entity at tile center.

### 6. Generator checksum

For debug/test builds, hash the final base tile-kind byte array + sorted treasure assignments. Store expected hashes for fixture seeds.

## Determinism tests

Required fixtures:

```text
seed=1, players=2
seed=123456789, players=4
seed=0xFFFFFFFF, players=8
```

For each fixture assert:

- exact base map hash;
- exact spawn assignment;
- exact treasure indices;
- no interior rock is orthogonally adjacent to another generated interior rock;
- all spawn safe-zone cells are floor;
- treasure does not overlap spawn safe zones/rock.

## Seed creation

Server creates a 32-bit seed using cryptographic random bytes when a round starts:

```ts
const bytes = crypto.getRandomValues(new Uint32Array(1));
const seed = bytes[0] >>> 0;
```

The seed itself is not secret; it is sent to clients for deterministic reconstruction.
