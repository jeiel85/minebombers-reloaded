# 11. Production Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| GitHub Pages cannot host realtime server logic | Blocking | Keep only static client on Pages; WSS goes to Cloudflare |
| Free-tier quota exhaustion | Matches fail | usage dashboard, lower input frequency, input-on-change, paid plan if public scale grows |
| Latency | Poor movement | client prediction + reconciliation + remote interpolation |
| Client cheating | Corrupt matches | server-authoritative state and validation |
| Host quits | Room disruption | host role transfer; server stays authoritative |
| DO restart/eviction | Lost live state | small checkpoints + resync path; use hibernation attachments for session metadata |
| Protocol deploy mismatch | Disconnects | protocol version gate and compatible rollout |
| Large snapshots | Bandwidth/GC | seeded static map, compact dynamic snapshots, later deltas/binary |
| Browser tab sleep | stale client | server neutralizes on disconnect; resume/full resync |
| Mobile network instability | disconnect churn | backoff + grace period + token resume |
| Original game IP/assets | redistribution risk | original assets/branding for public release unless rights are confirmed |

## Key architectural boundary

Do **not** put game rules into Phaser scene objects only. The authoritative rule engine must be reusable in a headless environment so the server can validate outcomes independently.

Recommended dependency direction:

```text
shared protocol/types  <- client
shared pure game defs  <- client
shared pure game defs  <- server
server authoritative engine must not import Phaser/DOM
```

## What not to add in v1

Avoid until core round works:

- accounts/login;
- global matchmaking;
- persistent rankings;
- chat moderation system;
- WebRTC mesh;
- cosmetic store;
- complex anti-cheat fingerprinting;
- replay video rendering.

Each adds operational surface without proving the core game.
