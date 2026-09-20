# 13. Graphics Asset Plan

## Decision

Do **not** ship copied Mine Bombers sprites, tiles, UI, logos, maps or audio. The project will use original assets with a DOS-era mining/demolition mood, while preserving only high-level gameplay inspiration.

Freeware redistribution of the original executable is not treated as permission to reuse its individual art/audio in a new derivative work.

## Visual direction

Working direction: **retro industrial mine + demolition arcade**.

- top-down orthographic view;
- crisp pixel art;
- low-detail silhouettes that remain readable at small size;
- dark rock/soil environment with brighter players, explosives and treasure;
- no visual tracing of the original game's sprites or UI;
- 8 player colors from palette/tint rather than eight independent character sheets where possible.

## Base rendering contract

```text
Logical tile:          32 x 32 px
World grid:            31 x 23 tiles by default
Native world view:     992 x 736 px
Player sprite cell:    32 x 32 px
Player collision AABB: 18 x 18 px equivalent, centered at feet/body core
Pickup sprite cell:    32 x 32 px
Bomb sprite cell:      32 x 32 px
Explosion cell:        32 x 32 px
UI icon:               24 x 24 or 32 x 32 px
Filtering:             nearest-neighbor
Camera scale:          integer scale when viewport permits
```

The simulation never uses sprite dimensions for collision. It uses world-unit hitboxes from the game constants.

## Asset phases

### Phase A — functional placeholders

Use geometric placeholder PNG sheets generated specifically for this bundle so gameplay work is not blocked by art. They contain no copied Mine Bombers source artwork.

The bundle includes placeholder sheets under:

```text
assets/placeholders/
```

They are deliberately simple and may be replaced without code changes.

### Phase B — visual prototype

Create one production-quality vertical slice containing:

- floor/soil/rock/breakable material;
- one miner character with 4-direction walk;
- timed bomb;
- explosion center/arms/end caps;
- treasure pickup;
- one shop panel and 4 equipment icons.

Playtest readability before creating the entire set.

### Phase C — production set

Complete all items in `assets/spec/asset-manifest.json` using the exact keys and frame contracts. Final assets may be hand-authored in Aseprite/LibreSprite or produced from concept art and then manually pixel-cleaned. Generative images should be treated as concept/reference material, not blindly used as final sprite sheets, because frame consistency and exact pixel-grid alignment are critical.

## Tileset contract

`tiles_world.png` is a uniform 32x32 spritesheet.

Initial frame IDs:

| Frame | Key | Collision | Destructible | Notes |
|---:|---|---|---|---|
| 0 | floor | no | no | exposed tunnel/floor |
| 1 | soil | yes while undug | yes by digging/explosive | primary mine material |
| 2 | rock | yes | no | hard boundary/pillar |
| 3 | cracked_soil | yes | yes | optional damage feedback |
| 4 | hazard | no | no | visual hazard marker; game rule separate |
| 5 | shop_marker | no | no | only for special maps |
| 6 | spawn_marker | no | no | editor/debug only |
| 7 | void | yes | no | out-of-play/black |

Do not encode authoritative collision purely from tile frame number on the client. The shared map definition owns tile semantics.

## Character sheet contract

`miner.png`:

```text
cell: 32 x 32
columns: 4 animation frames
rows: down, left, right, up
frame rate: 8 fps walking
idle: frame 0 of current direction
```

Prefer a neutral/light body layer that can be tinted to the player's assigned palette. Equipment that changes silhouette should use overlay sprites rather than duplicate all base animations.

## Explosions

Explosions are assembled from tile-sized segments rather than one giant image:

- center;
- horizontal middle;
- horizontal end;
- vertical middle;
- vertical end;
- optional debris/smoke particles.

Each segment uses 4 animation frames at 12–16 fps. Server sends the explosion event and authoritative affected tile set; client only animates it.

## Texture organization

For the first version, uniform spritesheets are simpler than a packed atlas:

```text
public/assets/gfx/
  tiles_world.png
  miner.png
  bombs.png
  explosions.png
  pickups.png
  ui_icons.png
```

When the asset count grows, pack non-uniform UI/equipment images into a texture atlas while keeping the world tileset as a uniform spritesheet. Phaser supports sprite sheets, tile sets and texture atlases.

## Optional external prototyping assets

If an external pack is used temporarily, use only assets with a clear license and record every file in the manifest. CC0 packs such as Kenney can be used for prototypes, but the preferred final direction is a cohesive original set.

## Licensing manifest

Every shipped asset must record:

```json
{
  "id": "miner.base",
  "file": "public/assets/gfx/miner.png",
  "source": "generated-for-project",
  "author": "generated placeholder",
  "license": "generated-for-project-no-third-party-source",
  "modified": false
}
```

For external assets record source URL, original author, exact license and modifications. Never add an asset to production without a manifest entry.

## Replacement rule

Gameplay code references asset keys and frame names only. It must not depend on file-specific pixel colors or hand-coded crop coordinates outside the central asset registry. Therefore placeholder -> final art is a content replacement, not a gameplay refactor.
