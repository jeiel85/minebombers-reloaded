# 10. Implementation Plan

The order below is intended to produce a playable vertical slice early without deferring critical networking decisions.

## Milestone 0 — repository/build foundation

- npm workspaces;
- strict TypeScript typecheck in all packages;
- lint + unit test runner;
- Vite client starts;
- Wrangler local server starts;
- generated Cloudflare binding types (`wrangler types`) preferred;
- CI builds from a committed lockfile;
- copy placeholder art into client public assets.

Exit: `npm run typecheck && npm test && npm run build` passes.

## Milestone 1 — room vertical slice

- create room;
- validate existing room on join;
- Hibernation WebSocket accept;
- strict hello validation;
- lobby roster;
- ready state;
- max 8 connected+reserved slots;
- token-hash resume;
- 20 sec grace;
- host transfer after grace.

Exit: 4 tabs join, one refreshes and resumes same identity.

## Milestone 2 — deterministic headless world

Implement without Phaser dependency:

- seeded map generator;
- player spawn;
- movement/AABB collision;
- soil digging;
- treasure placement/pickup;
- fixed-step `stepWorld`;
- deterministic recorded-input tests.

Exit: same seed + same input stream => same final hash.

## Milestone 3 — active server loop

- 20 Hz server-owned timer during `playing`;
- 10 Hz max snapshots;
- input freshness timeout;
- 2 sec checkpoint;
- restore active checkpoint after object restart;
- stop timer immediately outside playing.

Exit: round timer and due explosive scheduler progress without movement packets.

## Milestone 4 — real Phaser client

- Boot/Menu/Lobby/Shop/Game/Result scenes;
- load included placeholders;
- map renderer;
- player animation/tint;
- create/join UI;
- GameSocket + reconnect;
- prediction/interpolation;
- debug overlay.

Exit: two remote browsers can move and mine with simulated 150 ms RTT.

## Milestone 5 — one complete Classic round

Only one explosive first:

- Small Charge purchase;
- authoritative placement;
- fuse;
- blast propagation;
- soil destruction;
- damage/death;
- treasure/cash;
- round end;
- result -> shop.

Exit: 10 consecutive rounds without manual reset.

## Milestone 6 — match economy

- 5-round match;
- persistent match cash;
- pickaxe upgrade;
- Heavy Charge;
- Proximity Mine;
- Med Kit;
- final tie rules.

Exit: full match can finish/rematch with no state leak.

## Milestone 7 — production graphics/audio pass

- replace placeholder sheets using fixed manifest contract;
- verify nearest-neighbor scaling;
- visual readability at full-map fit;
- original explosion/dig/pickup/UI SFX;
- license manifest complete.

## Milestone 8 — hardening/deploy

- token bucket rate limiter;
- protocol fuzz tests;
- origin allowlist;
- Playwright multiplayer E2E;
- production Worker/DO;
- GitHub Pages Action;
- usage dashboards/log sampling;
- 8-player and reconnect soak tests.

## v1 definition of done

- 2–8 players join by room code;
- mining/treasure/shop loop works;
- 5-round Classic match finishes deterministically;
- combat/economy are server-authoritative;
- reconnect restores same identity within 20 sec;
- active match timing does not depend on client timers;
- 150 ms RTT is playable;
- placeholders can be replaced without gameplay refactor;
- all shipped assets have provenance/license entries;
- GitHub Pages contains no secret;
- Cloudflare usage stays measurable against current quotas.
