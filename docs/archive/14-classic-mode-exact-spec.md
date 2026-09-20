# 14. Classic Multiplayer Mode — Exact Baseline Specification

This is the v1 gameplay baseline. Values are deliberately explicit so implementation can begin without another design pass. Balance can change after playtesting without changing architecture.

## Match structure

```text
ROOM LOBBY
  -> SHOP (20 s or all ready)
  -> ROUND COUNTDOWN (3 s)
  -> PLAYING (default 300 s)
  -> ROUND RESULT (8 s)
  -> SHOP
  -> ...
  -> MATCH RESULT after configured round count
```

Default match: 5 rounds.

## Primary objective

Classic mode preserves the mining/economy loop:

- mine through soil;
- collect treasure/cash;
- kill opponents for a bounty;
- spend accumulated cash between rounds;
- highest cash after the configured round count wins by default.

A room option may use round wins instead, but cash-score is the default.

## Round end conditions

The server ends a round when the first condition occurs:

1. round timer reaches zero;
2. all treasure nodes have been collected;
3. fewer than two non-eliminated players remain and at least two players started the round.

If everyone is eliminated by the same explosion, the round still ends and no survivor bonus is given.

## Default economy

```text
starting cash:          500
basic treasure:         100
rare treasure:          300
player kill bounty:     400
survivor bonus:         150
```

Cash is server-authoritative and persists between rounds within the match only.

## Movement

Simulation uses integer world units:

```text
1 tile                 = 1024 world units
player collision box   = 576 x 576 world units (~18 px at 32 px/tile)
base move speed        = 3000 world units / second (~2.93 tiles/s)
SIM_STEP_MS            = 50
```

Diagonal movement is normalized so it is not faster than cardinal movement.

## Digging

Soil is not an ordinary impassable wall. When a player pushes into an adjacent soil tile:

1. server identifies the contacted soil tile;
2. server accumulates dig progress while movement intent remains toward that tile and player is within dig range;
3. once required dig work reaches zero, the soil becomes floor;
4. movement can continue into the opened tile.

Default dig work:

```text
soil durability:       1000 work units
base pickaxe rate:     1000 work units / second
```

Therefore one plain soil tile takes about 1 second with starter equipment. Upgrades multiply dig rate.

The server owns dig progress. The client may display predicted cracks/progress but cannot open a tile itself.

## Map baseline

```text
size:                  31 x 23 tiles
outer border:          indestructible rock
spawn regions:         near corners / evenly distributed edge regions
spawn safe radius:     2 tiles of guaranteed initial floor
soil density:          deterministic from seed
rock pillar density:   deterministic from seed
minimum connectivity:  generator must guarantee each spawn has at least one diggable route into central region
```

Map generation must be deterministic from `(generatorVersion, seed, playerCount)`.

## Treasure generation

Treasure is hidden in deterministic soil locations. The client receives only what should be visible according to game design. For v1 there is no secure fog-of-war requirement; hidden treasure locations may still be derivable from the seed, so do not treat client secrecy as anti-cheat.

Default per 31x23 map:

```text
basic treasure:        18 + 2 * playerCount
rare treasure:         max(2, playerCount)
```

## Health and death

```text
max HP:                100
spawn invulnerability: 1500 ms
```

A player at HP <= 0 is eliminated for the round. Their movement/actions are disabled and they become a spectator.

## Starter equipment

All equipment definitions live in data, not hard-coded switch statements.

### Pickaxe I

```text
owned by default
cost: 0
dig multiplier: 1.0
```

### Pickaxe II

```text
cost: 700
dig multiplier: 1.45
persists for remaining rounds of match
```

### Small Charge

```text
cost per unit: 120
fuse: 1800 ms
blast radius: 2 tiles cardinal
player damage: 70
soil: destroys
rock: unaffected
max active per player: 2
```

### Heavy Charge

```text
cost per unit: 300
fuse: 2400 ms
blast radius: 4 tiles cardinal
player damage: 100
soil: destroys
rock: blocks
max active per player: 1
```

### Proximity Mine

```text
cost per unit: 250
arming delay: 1000 ms
trigger radius: 0.75 tile
damage: 100
blast radius: 1 tile
max active per player: 2
```

### Med Kit

```text
cost per unit: 200
heal: 40
use time: immediate for v1
cannot exceed max HP
```

These are original baseline definitions; do not copy names/statistics from the old game.

## Shop rules

- shop occurs before round 1 and between rounds;
- purchases are requested by client and validated server-side;
- server checks cash, item stock limits and phase;
- purchase response includes authoritative cash/inventory;
- player can toggle ready;
- shop closes when all connected non-spectator players are ready or timer expires;
- disconnected reserved players are treated as not ready until grace expires; if grace expires, server continues without them.

## Bomb/charge propagation

Cardinal blast algorithm:

1. center tile is affected;
2. step one tile in each cardinal direction up to radius;
3. rock stops the ray before affecting tiles behind it;
4. soil is affected and stops the ray by default for Small Charge; Heavy Charge may continue one extra tile after destroying soil only if the equipment definition says `penetration = 1`;
5. another explosive in an affected tile is scheduled for chain detonation at `max(now + 50ms, currentSimulationTime + oneStep)`;
6. each explosion ID can damage a player at most once.

## Respawn

Classic mode has **no mid-round respawn**. Players return next round.

## Tie handling

Match cash tie:

1. most round wins;
2. most kills;
3. most treasure value collected;
4. remain a declared tie if still equal.

Do not use random tie breaking.
