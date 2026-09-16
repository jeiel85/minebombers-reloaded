# 16. Storage and Reconnect Contract

## Persistent room record

Persist one compact `RoomRecord` plus the active checkpoint. Do not persist every simulation tick.

```ts
interface RoomRecord {
  schemaVersion: 1;
  roomCode: string;
  createdAt: number;
  updatedAt: number;
  phase: RoomPhase;
  hostPlayerId: string | null;
  settings: RoomSettings;
  matchId: string | null;
  roundIndex: number;
}
```

## Player slot record

A room has at most 8 slots including reconnect-reserved players.

```ts
interface PlayerSlot {
  playerId: string;
  displayName: string;
  joinedAt: number;
  connected: boolean;
  ready: boolean;
  reservedUntil: number | null;
  resumeTokenHash: string;
  cash: number;
  inventory: InventoryState;
  stats: MatchStats;
}
```

Capacity check uses `slots.length`, not only connected sockets.

## Resume token

- generate >= 128 bits of cryptographically secure entropy;
- return raw opaque token once to that client;
- persist only a SHA-256 hash (or keyed HMAC if a Worker secret is introduced later);
- compare hashes in constant-time where practical;
- rotate token after successful resume so a captured old token cannot be reused indefinitely.

Client storage key example:

```text
minebombers.resume.<roomCode>
```

Use `sessionStorage` by default.

## Hello rules

`c.hello` is accepted once per WebSocket.

New join:

- room must exist and be joinable;
- no resume token;
- allocate slot only when `slots.length < MAX_PLAYERS`;
- normalize name;
- reject duplicate abuse on same socket.

Resume:

- room must contain matching unexpired reserved slot;
- hash must match;
- replace any stale socket association;
- mark connected;
- rotate token;
- send `s.welcome` followed by full authoritative state.

## Disconnect

On close/error:

1. neutralize movement and held action state immediately;
2. mark slot disconnected;
3. set `reservedUntil = now + 20_000`;
4. retain entity according to game phase;
5. schedule/coalesce cleanup check;
6. broadcast roster/connection state.

## Host behavior

Lobby/shop host reservation follows reconnect grace. Do not transfer host immediately on transient disconnect.

After grace expires:

- remove expired slot when allowed by phase;
- choose oldest connected slot as host;
- broadcast host change.

During `playing`, host has no game authority and host changes do not affect simulation.

## Uninitialized room protection

`/api/rooms/:code/ws` and `/status` must verify a persisted `createdAt`/RoomRecord. A Durable Object identity existing due to `idFromName()` is not itself proof that a room was created.

Return `404 ROOM_NOT_FOUND` for arbitrary uninitialized codes.

## Checkpoint storage keys

Recommended:

```text
room              RoomRecord
players           PlayerSlot[]
checkpoint        ActiveRoundCheckpoint | null
expiresAt         number
```

Use multi-key operations where appropriate. Avoid one storage row per dynamic entity for the MVP; the complete checkpoint is small at 2–8 players and a 31x23 grid.
