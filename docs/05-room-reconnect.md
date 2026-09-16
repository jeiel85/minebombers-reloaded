# 05. Room Lifecycle, Host Role and Reconnect

Detailed storage/token rules are in `16-storage-reconnect-contract.md`.

## Phases

```ts
type RoomPhase =
  | 'lobby'
  | 'shop'
  | 'countdown'
  | 'playing'
  | 'round_result'
  | 'match_result'
  | 'closed';
```

All phase transitions occur on the server.

## Host

Host is a **lobby permission role only**. The host may start/configure the room where allowed. Host has no combat authority.

## Reconnect grace

```text
RECONNECT_GRACE_MS = 20_000
```

On disconnect:

- neutralize movement immediately;
- retain player slot;
- mark disconnected;
- reserve slot until grace deadline;
- preserve their entity during the active round and leave it vulnerable;
- reject new players if all 8 slots are connected or reserved.

## Resume flow

1. browser reconnects with opaque token;
2. server hashes and matches token to an unexpired reserved slot;
3. server rotates token;
4. same `playerId`, match economy and inventory are restored;
5. full authoritative state is sent;
6. client discards old prediction history and resumes.

## Host transfer

In lobby/shop/result phases, transient host disconnect does **not** immediately transfer host. Keep reservation until reconnect grace expires. If it expires, assign oldest connected player.

During a round, host transfer has no simulation effect.

## Room expiry

Use a coarse Durable Object alarm for cleanup, not a high-frequency game loop.

Suggested policy:

```text
empty lobby/shop:       30 min
empty match result:     10 min
closed room:            delete state promptly
```

## Browser lifecycle

- `visibilitychange`: keep socket and game running; do not pause multiplayer;
- `pagehide`: best-effort close only;
- sleep/wake/network change: reconnect + full resync;
- tab background throttling must not control authoritative timers because the server runs the active round loop.
