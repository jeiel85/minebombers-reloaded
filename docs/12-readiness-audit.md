# 12. Implementation Readiness Audit

Audit date: 2026-09-16

## Verdict

The original bundle was **architecture-ready but not implementation-complete**. A developer could start the repository and networking scaffold, but several decisions still had to be made during coding. This revision closes those design gaps so implementation can proceed without re-architecting the project.

## Gaps found in the first revision

| Area | Problem | Required correction |
|---|---|---|
| Active match timing | World advanced only when messages arrived; timed bombs could be delayed if no input arrived | Explicit active-match simulation driver; hibernation only outside active play |
| Mine Bombers identity | Baseline leaned too far toward generic last-man-standing Bomberman | Make mining, treasure, between-round shop and cumulative cash the primary Classic mode |
| Reconnect | Code regenerated player identity; resume token was not persisted | Reserved slot + token hash + grace deadline + full resync contract |
| Host transfer | Code transferred immediately but documentation specified grace | Keep host reservation through grace in lobby; transfer after expiry |
| Capacity | Disconnected reserved slots were not counted toward the room maximum | Capacity counts connected + reconnect-reserved players |
| Room existence | Direct `/ws` access could effectively hydrate an uninitialized room code | Require persisted room metadata before status/join |
| Protocol | Dynamic entities/events were `unknown[]` | Define concrete entity/event unions before gameplay implementation |
| Validation | `parseClientMessage()` only checked `t` | Strict runtime validation and range limits are mandatory |
| Client | No create/join/lobby flow and no actual game socket use | Explicit scene/UI flow and reconnect state machine |
| Simulation | No exact movement/digging/hitbox/bomb constants | Exact baseline constants and deterministic rules added |
| Graphics | No asset manifest, dimensions, frame map or replacement strategy | Original-asset pipeline + placeholder pack + fixed asset contract |
| Build quality | No client `tsc --noEmit`, tests or lint implementation | Add typecheck/test/lint before public deploy |
| Cost model | Hibernation benefits were discussed without separating lobby vs active-match timers | Separate hibernating lobby from pinned active match and model both request + duration limits |

## Required implementation gates

Do not call the game production-ready until all gates are green:

1. `npm run typecheck` passes in client, server and shared packages.
2. Protocol validators reject malformed input before switch dispatch.
3. Headless deterministic simulation passes recorded-input fixtures.
4. Two browsers can create, join, shop, play one full round and rematch.
5. Refresh during a round resumes the same player within the grace window.
6. A player cannot create a ninth reserved slot by disconnect/reconnect abuse.
7. Bombs detonate on server schedule even when players send no movement changes.
8. 150 ms simulated RTT remains playable with prediction/interpolation.
9. All shipped visual/audio assets have entries in the asset/license manifest.
10. Production GitHub Pages origin is the only browser origin allowed by the Worker, plus explicit localhost development origins.

## Implementation status terminology

- **Design-complete**: all behavior and contracts needed for implementation are specified.
- **Starter scaffold**: repository and network skeleton; not the full game.
- **Playable vertical slice**: room -> shop -> round -> result works with one bomb and mining.
- **v1**: complete Classic mode with reconnect, shop, multiple equipment items, deployment and hardening.

This bundle is intended to be **design-complete** and to provide a starter scaffold. It is not represented as a finished game implementation.
