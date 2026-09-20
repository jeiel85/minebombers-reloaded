# 18. File-by-File Implementation Map

This is the recommended coding order for the starter repository.

## Shared package

Add:

```text
packages/shared/src/game/constants.ts
packages/shared/src/game/types.ts
packages/shared/src/game/equipment.ts
packages/shared/src/game/prng.ts
packages/shared/src/game/mapGenerator.ts
packages/shared/src/protocol/client.ts
packages/shared/src/protocol/server.ts
packages/shared/src/protocol/validate.ts
```

Responsibilities:

- deterministic pure data/types;
- no Phaser imports;
- no Cloudflare imports;
- map generator shared so client can reconstruct base map;
- strict protocol validation;
- equipment definitions.

## Server package

Add:

```text
apps/server/src/room/GameRoom.ts
apps/server/src/room/RoomStorage.ts
apps/server/src/room/SimulationLoop.ts
apps/server/src/game/WorldState.ts
apps/server/src/game/stepWorld.ts
apps/server/src/game/movement.ts
apps/server/src/game/digging.ts
apps/server/src/game/explosions.ts
apps/server/src/game/economy.ts
apps/server/src/game/shop.ts
apps/server/src/game/snapshots.ts
apps/server/src/security/RateLimiter.ts
```

`GameRoom` should orchestrate; do not put all gameplay logic into the Durable Object class.

## Client package

Add:

```text
apps/client/src/scenes/BootScene.ts
apps/client/src/scenes/MenuScene.ts
apps/client/src/scenes/LobbyScene.ts
apps/client/src/scenes/ShopScene.ts
apps/client/src/scenes/GameScene.ts
apps/client/src/scenes/ResultScene.ts
apps/client/src/net/GameSocket.ts
apps/client/src/net/ClockSync.ts
apps/client/src/net/Reconnector.ts
apps/client/src/game/Prediction.ts
apps/client/src/game/Interpolation.ts
apps/client/src/game/AssetRegistry.ts
apps/client/src/game/InputController.ts
apps/client/src/ui/DebugOverlay.ts
```

## Tests

Add:

```text
packages/shared/test/mapGenerator.test.ts
apps/server/test/movement.test.ts
apps/server/test/digging.test.ts
apps/server/test/explosions.test.ts
apps/server/test/economy.test.ts
apps/server/test/protocol.test.ts
apps/server/test/room.integration.test.ts
apps/client/e2e/multiplayer.spec.ts
```

## First vertical slice

Implement only these mechanics before adding more weapons:

1. create/join room;
2. lobby ready/start;
3. shop with Small Charge purchase;
4. deterministic map;
5. movement and digging;
6. treasure collection;
7. Small Charge placement/detonation;
8. damage/death;
9. round result;
10. next shop/rematch;
11. reconnect during playing.

Only after this slice is stable add Heavy Charge, mine, med kit and richer graphics.
