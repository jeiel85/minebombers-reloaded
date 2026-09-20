# 02. Gameplay Specification

This project is a **Mine Bombers-inspired multiplayer mining/demolition game**, not a direct content clone. Original graphics, sound, maps, branding and source code are not reused.

The authoritative v1 gameplay baseline is defined in `14-classic-mode-exact-spec.md`. This file summarizes the architecture-facing rules.

## Core loop

```text
CREATE/JOIN
 -> LOBBY
 -> SHOP
 -> ROUND
 -> ROUND RESULT
 -> SHOP
 -> ...
 -> MATCH RESULT
```

The identity of the game comes from **mining + treasure + combat + between-round shopping**, not from a generic last-player-standing bomb arena.

## Default match

```text
players:               2–8
rounds:                5
round duration:        300 sec
map:                   31 x 23 tiles
render tile:           32 x 32 px
simulation coordinate: fixed integer world units
1 tile:                1024 world units
winner metric:         cash after final round
```

Round ends on timeout, all treasure collected, or fewer than two starting players remain alive.

## World tile kinds

```ts
type TileKind =
  | 'floor'
  | 'soil'
  | 'rock'
  | 'hazard'
  | 'spawn';
```

- `floor`: traversable;
- `soil`: blocks movement until mined/destroyed;
- `rock`: indestructible and blocks blasts;
- `hazard`: traversability/damage defined by map rule;
- `spawn`: semantic generator marker, rendered as floor during play.

## Mining

Walking into soil begins authoritative dig work rather than treating soil as an ordinary wall. Dig progress belongs to the server. Equipment changes dig rate.

See `14-classic-mode-exact-spec.md` for exact durability and starter equipment values.

## Economy

Server owns:

- cash;
- treasure pickup;
- kill bounty;
- shop purchases;
- inventory;
- persistent equipment upgrades within the match.

Client sends purchase intent only.

## Equipment data model

```ts
interface EquipmentDef {
  id: string;
  kind: 'tool' | 'explosive' | 'mine' | 'consumable';
  cost: number;
  maxOwned?: number;
  maxActive?: number;
  cooldownMs?: number;
  fuseMs?: number;
  blastRadiusTiles?: number;
  damage?: number;
  digMultiplier?: number;
}
```

Values live in a central data table, not scattered through scene/server classes.

## Explosions

Explosion consequences are server-authoritative and deterministic:

1. center affected;
2. cardinal rays advance tile-by-tile;
3. rock stops propagation;
4. soil may be destroyed and normally stops the ray unless equipment grants penetration;
5. explosives hit by a blast chain-detonate on a later simulation step;
6. one explosion ID damages each player at most once;
7. client receives affected cells/events and only animates presentation.

## Randomness

Gameplay-affecting randomness uses a seeded PRNG and versioned generator. Never use `Math.random()` in authoritative game rules.

Seeded systems include:

- map terrain;
- spawn assignment;
- treasure placement;
- any randomized drops added later.

## Asset rule

Gameplay code uses semantic asset keys. Placeholder and production art share the same frame contract. See `13-graphics-assets.md` and `assets/spec/asset-manifest.json`.
