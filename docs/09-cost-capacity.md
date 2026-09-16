# 09. Cost and Capacity Model

**Verified against official Cloudflare documentation on 2026-09-16. Re-check before launch.**

## Current documented Free figures

Cloudflare Durable Objects currently documents:

- 100,000 requests/day;
- 13,000 GB-s duration/day;
- 20 incoming WebSocket messages count as 1 request for compute request billing;
- outgoing WebSocket messages are not request-billed;
- SQLite-backed Durable Objects are available on Free;
- Free SQLite storage limit of 5 GB total;
- exceeding a Free limit can cause further operations of that type to fail until reset.

Sources:

- https://developers.cloudflare.com/durable-objects/platform/pricing/
- https://developers.cloudflare.com/durable-objects/platform/limits/

## Lifecycle matters

A room has two cost modes:

### Hibernatable

Lobby/shop/result can use WebSocket Hibernation with no repeating JavaScript timer. When idle and eligible to hibernate, duration billing stops.

### Active round

A real-time authoritative round uses a scheduled JavaScript callback for the 20 Hz simulation. Scheduled callbacks prevent hibernation, so the room consumes duration for the active round. This is intentional for deterministic server-owned bomb/round timing.

## Duration planning

Cloudflare duration examples use 128 MB (= 0.125 GB) per Durable Object.

```text
13,000 GB-s / 0.125 GB = 104,000 active seconds/day
104,000 / 3600 ≈ 28.9 active room-hours/day
```

That is a theoretical duration-only ceiling, before other usage/limits.

For an 8-minute active round:

```text
8 min = 480 sec
480 * 0.125 = 60 GB-s per active room-round
13,000 / 60 ≈ 216 such rounds/day from duration alone
```

## Request traffic with input-on-change

Do **not** send 10 identical movement packets/sec forever. The server owns the tick.

Baseline policy:

```text
movement state changes: immediate
held-state refresh:     every 500 ms
ping:                   every 5 s
combat/shop actions:    discrete immediate messages
```

Example worst-ish held movement refresh for 4 players over one 8-minute round:

```text
4 players * 2 refreshes/sec * 480 sec = 3,840 incoming movement messages
3,840 / 20 = 192 request-equivalents
```

Actual total is higher because of actions, pings, joins and state changes, but much lower than a fixed 10 Hz input stream.

## Snapshot traffic

Snapshots are outbound. Keep <= 10 Hz and include dynamic state only. Static terrain derives from seed/version; changed tiles are deltas/full resync as needed.

## Checkpoint storage

At 2-second checkpoint cadence an 8-minute round performs roughly 240 checkpoint writes per active room if every checkpoint is persisted. That can become a meaningful Free-tier row-write cost at scale. Optimize after correctness by:

- checkpointing every 3–5 sec if recovery quality remains acceptable;
- skipping writes when no meaningful state changed;
- storing one compact checkpoint row rather than one row per entity.

## GitHub Pages

Current GitHub documentation states a published Pages site may be up to 1 GB and has a soft bandwidth limit of 100 GB/month. Keep the game bundle small and cacheable.

Source:

- https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits

## Scaling decision

For a private/friends release, this architecture is reasonable. If concurrency grows enough that Durable Object active duration becomes material, measure before changing architecture. A WebRTC hybrid is a later optimization, not a v1 requirement.
