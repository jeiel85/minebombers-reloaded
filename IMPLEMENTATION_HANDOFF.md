# Implementation Handoff

Use this file when handing the bundle to a coding agent/developer.

## Objective

Implement the supplied design as a playable 2–8 player browser game without changing the deployment architecture:

```text
GitHub Pages client
+ Cloudflare Worker router
+ one Durable Object per room
```

## Non-negotiable rules

1. Server owns movement validation, digging completion, inventory, money, damage, explosives and results.
2. Active rounds have a server-owned 20 Hz fixed-step loop; lobby/shop/result may hibernate.
3. Do not add a VM, long-running dedicated host, Firebase or Supabase unless the architecture is explicitly reconsidered later.
4. Do not copy original Mine Bombers graphics/audio/code/maps/branding.
5. Use the included 32x32 placeholders until final original art replaces them.
6. Implement `classic-mine-v1` exactly before changing generator behavior.
7. Resume must preserve the same player identity within the 20-second grace window.
8. Capacity includes reconnect-reserved slots.
9. Add tests before adding extra weapons beyond the first complete Small Charge vertical slice.

## Read first

- `docs/12-readiness-audit.md`
- `docs/14-classic-mode-exact-spec.md`
- `docs/15-server-timing-cost.md`
- `docs/16-storage-reconnect-contract.md`
- `docs/19-map-generator-determinism.md`
- `docs/18-file-by-file-implementation.md`

## First playable target

A successful first vertical slice is complete when:

1. Player A creates a room and receives a code.
2. Player B joins that code from another browser.
3. Both see lobby roster and ready.
4. Shop allows Small Charge purchases from server-owned cash.
5. Round starts on the same deterministic generated mine.
6. Both can move and dig soil.
7. Treasure collection changes authoritative cash.
8. Small Charge destroys soil and damages/eliminates a player.
9. Round ends and returns to shop.
10. Refresh during the round reconnects to the same player and state.

Do not expand scope until this path has integration/E2E coverage.


## Verification status

This bundle is **implementation-spec ready**, not a finished game. The exact gameplay, timing, protocol, storage/reconnect, map-generation, scene flow, graphics contract and file ownership are specified so implementation can proceed without reopening architecture decisions.

Verification performed in this workspace:

- shared TypeScript package: `tsc --noEmit` passes with the available compiler
- JSON manifests parse successfully
- generated placeholder PNG dimensions match the asset contract
- stale `unknown[]` protocol placeholders were removed
- active-round timing no longer depends on client traffic

Environment limitation: a complete fresh `npm install` for the whole monorepo did not finish within the execution environment's network/time limit. Therefore the client and Cloudflare Worker have **not** been end-to-end dependency-installed and built here. Run `npm install`, `npm run typecheck`, client build, and Wrangler local smoke test as Milestone 0 before gameplay implementation.
