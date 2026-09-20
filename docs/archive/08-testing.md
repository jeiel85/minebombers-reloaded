# 08. Testing Plan

## Test pyramid

### Unit tests — shared simulation

Must cover:

- tile collision;
- movement limits;
- bomb propagation;
- chain detonation;
- breakable wall destruction;
- damage idempotency;
- loot PRNG reproducibility;
- cooldown/inventory validation.

Simulation logic should be pure functions where possible.

### Protocol tests

- every message schema accepts valid payloads;
- invalid enum/range rejected;
- oversized names rejected;
- duplicate action sequence ignored/rejected;
- old protocol version rejected.

### Durable Object integration tests

Scenarios:

1. create room -> join two clients -> ready -> start;
2. third client receives current lobby roster;
3. input updates authoritative state;
4. bomb event reaches all clients;
5. host disconnect -> host transfer;
6. player reconnect with token;
7. reconnect with wrong token rejected;
8. full room rejects 9th player;
9. malformed/rate-abusive client is disconnected;
10. state reload from checkpoint.

Use Cloudflare's current recommended Workers/Durable Objects test tooling at implementation time.

## Browser E2E

Use Playwright with 2–4 browser contexts:

- create/join by room code;
- ready/start UI;
- simultaneous movement;
- bomb placement;
- refresh/reconnect;
- results -> rematch.

## Network simulation

Test at least:

```text
RTT:             20 / 80 / 150 / 250 ms
jitter:          0 / 20 / 50 ms
packet behavior: WebSocket reliable, but emulate delayed/batched delivery
reconnect:       Wi-Fi toggle / browser offline / sleep-wake
```

Acceptance goals are gameplay-specific, but obvious position oscillation or repeated hard snaps at ~100–150 ms RTT indicates prediction/reconciliation needs tuning.

## Load test

Do not start with huge synthetic load. First prove:

- 8 sockets in one room;
- several concurrent rooms;
- message rate at configured limits;
- memory/state growth bounded;
- no room cross-talk.

Then scale synthetic rooms while monitoring Cloudflare request/duration usage.

## Determinism regression

Record a stream of inputs + seed and replay it against the shared simulation. The final state hash should match expected fixtures. This is extremely useful after changing collision or weapon rules.
