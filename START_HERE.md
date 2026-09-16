# Starter Scaffold — Start Here

This folder is a **network/shared-game scaffold**, not the finished game.

It now includes:

- npm workspace layout;
- Cloudflare Worker + Durable Object WebSocket skeleton;
- concrete v1 protocol unions and baseline runtime validator;
- deterministic `classic-mine-v1` map generator;
- shared gameplay/equipment constants;
- GitHub Pages workflow;
- original geometric placeholder spritesheets.

## Copy into a repository

Copy the contents of `starter/` to the repository root.

Then:

```bash
npm install
```

Commit the generated `package-lock.json`, then use `npm ci` in CI for reproducible installs.

## Typecheck

```bash
npm run typecheck
```

Cloudflare currently recommends generated binding types. During the first setup pass run the current Wrangler type generation command and migrate the server tsconfig/types accordingly.

## Run server

```bash
npm run dev:server
```

## Run client

Create `apps/client/.env.local`:

```text
VITE_API_BASE=http://localhost:8787
```

Then:

```bash
npm run dev:client
```

## Implement next, in order

1. Room existence check + persistent `RoomRecord` / `PlayerSlot` storage.
2. Resume-token hashing/rotation and 20-second reserved slots.
3. Room phase state machine: lobby -> shop -> countdown -> playing -> result.
4. Active 20 Hz authoritative round loop.
5. `WorldState`, movement/AABB collision and digging.
6. Treasure pickup/economy.
7. Small Charge placement/explosion/damage.
8. Phaser Menu/Lobby/Shop/Game/Result scenes using the included placeholders.
9. Prediction/interpolation/reconnect UI.
10. Integration/E2E tests before adding the remaining equipment.

The exact behavior is specified in the parent bundle's `IMPLEMENTATION_HANDOFF.md` and `docs/12` through `docs/19`.

## Asset files

```text
apps/client/public/assets/gfx/
  tiles_world.png
  miner.png
  bombs.png
  explosions.png
  pickups.png
  ui_icons.png
```

These are development placeholders. Do not couple game logic to their colors; use the central asset/frame contract.

## Package versions

Versions verified on 2026-09-16:

- Phaser 4.2.1
- Vite 8.3.0
- Wrangler 4.132.0

Re-check versions before a later public release.
