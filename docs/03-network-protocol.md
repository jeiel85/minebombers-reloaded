# 03. Network Protocol

## Transport

- WebSocket over TLS (`wss://`) in production.
- JSON for v1 debuggability.
- Protocol version negotiated by `c.hello` / `s.welcome`.
- Binary transport is an optimization only after profiling.
- Client may never send authoritative position, HP, cash, inventory totals, damage, tile state or results.

The starter's `packages/shared/src/protocol.ts` contains the concrete v1 TypeScript union definitions and baseline runtime parser.

## Client -> server

### `c.hello`

```json
{
  "t": "c.hello",
  "v": 1,
  "name": "player",
  "clientBuild": "0.1.0",
  "resumeToken": null
}
```

Exactly once per WebSocket. Resume uses the same message with the opaque token.

### `c.ready`

Used in lobby/shop readiness where the current phase allows it.

```json
{ "t": "c.ready", "ready": true }
```

### `c.start`

Host request only. Server validates all start preconditions.

```json
{ "t": "c.start" }
```

### `c.input`

Movement state message. Send immediately when state changes and refresh unchanged held state roughly every 500 ms.

```json
{
  "t": "c.input",
  "seq": 381,
  "clientTime": 1760000000000,
  "dx": 1,
  "dy": 0,
  "primary": false,
  "secondary": false,
  "slot": 0
}
```

The server does not trust `clientTime` for game rules.

### `c.action`

Discrete exactly-once-ish intent guarded by monotonically increasing sequence number.

```json
{
  "t": "c.action",
  "seq": 94,
  "action": "place_bomb",
  "slot": 0
}
```

Allowed baseline actions:

- `place_bomb`
- `use_secondary`
- `interact`
- `use_item`

### `c.buy`

Shop purchase request.

```json
{
  "t": "c.buy",
  "seq": 95,
  "equipmentId": "small_charge",
  "quantity": 2
}
```

Server validates phase, price, cash, inventory cap and item definition.

### `c.ping`

```json
{ "t": "c.ping", "n": 42, "sentAt": 1760000000000 }
```

### `c.resync`

```json
{ "t": "c.resync", "reason": "snapshot_gap", "lastServerSeq": 9021 }
```

## Server -> client

### `s.welcome`

Returns room state, stable player ID and a freshly issued/rotated resume token.

### `s.roster`

Lobby/shop membership, ready and host changes.

### `s.shop`

Contains authoritative local cash, inventory, offers and shop close deadline.

### `s.purchase_result`

Correlates to the client's purchase sequence and returns authoritative cash/inventory whether accepted or rejected.

### `s.start`

```json
{
  "t": "s.start",
  "matchId": "m_...",
  "roundIndex": 1,
  "seed": 184739201,
  "startsAt": 1760000003000,
  "endsAt": 1760000303000,
  "mapGenerator": "classic-mine-v1"
}
```

### `s.snapshot`

A compact dynamic authoritative snapshot at up to 10 Hz.

Contains:

- server sequence/time;
- recipient's acknowledged input sequence;
- room phase/round deadline;
- players;
- dynamic entities such as explosives/treasure/pickups;
- changed tiles.

Static base terrain is reproducible from generator version + seed. Full resync may include all current tile deltas.

### `s.event`

Concrete `GameEvent[]`, including:

- explosive placement;
- explosion affected cells;
- tile changed;
- treasure collected;
- damage;
- elimination;
- cash changed;
- phase/round end.

### `s.round_result` / `s.match_result`

Authoritative standings. Client only renders them.

### `s.pong`

Clock sync / RTT estimate.

### `s.error`

Machine-readable code + safe human-readable message.

## Ordering

WebSocket preserves transport order, but application sequence numbers remain required for replay/duplicate protection and reconciliation.

Maintain separately:

```text
client input seq
client action/purchase seq
server event/state seq
```

## Rate policy baseline

```text
movement changes       burst 20, sustained behavior validated
held input refresh     ~2/sec while held
combat actions         <= 8/sec, burst 12
purchases              <= 5/sec, burst 8
ping                    ~1 per 5 sec
application frame      <= 16 KiB hard cap
```

Use per-socket token buckets. Persistent abuse can close the socket with policy violation.

## Runtime validation

TypeScript types alone are insufficient because browser clients are untrusted. Parse JSON to `unknown`, validate message type and every authoritative-relevant range, then cast/dispatch.

Normalize/sanitize display names separately after schema validation.

## Resync boundary

A full resync clears client interpolation/prediction histories that predate the returned state. The server remains authoritative; a client state hash is diagnostic only.
