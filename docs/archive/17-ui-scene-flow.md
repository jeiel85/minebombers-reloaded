# 17. Client UI and Scene Flow

## Phaser scenes / UI states

```text
BootScene
 -> MenuScene
 -> LobbyScene
 -> ShopScene
 -> GameScene
 -> ResultScene
```

A lightweight HTML overlay may be used for forms (display name, room code) if it improves accessibility. The game world stays in Phaser.

## MenuScene

Actions:

- Create room
- Join room
- Display name
- Audio settings
- Controls help

Create flow:

1. `POST /api/rooms`.
2. Receive room code.
3. Open WebSocket.
4. Send hello.
5. Transition to Lobby.

Join flow:

1. normalize code to uppercase and remove spaces/hyphens;
2. optionally `GET /status` for friendly error;
3. open WebSocket;
4. send hello;
5. transition on `s.welcome`.

## LobbyScene

Show:

- room code + copy button;
- player list with connection/ready state;
- host marker;
- round count and win metric;
- ready button;
- start button for host when server says start is legal.

Never infer that start is legal only from client state; server is authoritative.

## ShopScene

Layout:

- current cash prominent;
- equipment cards with price, stock owned and short description;
- inventory/loadout strip;
- ready button and countdown;
- purchase errors inline.

All purchase buttons send requests; UI updates to authoritative purchase response/event.

## GameScene

Layers back-to-front:

1. floor;
2. soil/rock tile layer;
3. hidden/revealed treasure layer;
4. explosives/pickups;
5. players;
6. explosion/effects;
7. HUD;
8. reconnect/error overlay.

HUD:

```text
HP | Cash this match | selected item/ammo | round timer | player count
```

Debug build adds RTT, server offset, server seq, ack input seq, prediction error and snapshot rate.

## Camera

Default desktop goal is to show the whole 31x23 arena at native 992x736 when screen permits. On smaller screens, scale to fit rather than grant a gameplay advantage by showing a larger world area to users with larger monitors.

For v1, all clients use the same logical visible arena bounds. Do not make competitive visibility depend on browser window size.

## Input map

```text
WASD / arrows  movement
Space          selected primary explosive/action
E              interact / use secondary
1..4           inventory slot
Tab            score overlay
Esc            local menu (does not pause multiplayer)
```

Use `KeyboardEvent.code` mapping on client but transmit semantic actions only.
