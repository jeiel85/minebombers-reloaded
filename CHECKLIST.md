# Implementation Checklist

## Foundation

- [x] Create repository from `starter/`
- [x] Add lockfile and use `npm ci` in CI
- [x] strict TypeScript typecheck: client/server/shared
- [x] test/lint scripts
- [x] `wrangler dev` starts
- [x] Vite client starts
- [x] `wrangler types` generated bindings
- [x] placeholder assets copied/loaded through central AssetRegistry

## Room/network

- [x] reject uninitialized room codes
- [x] strict runtime message validation
- [x] create/join room
- [x] hello only once/socket
- [x] lobby roster/ready
- [x] count connected + reserved slots toward max 8
- [x] hashed rotating resume token
- [x] 20 sec grace
- [x] host transfer only after grace
- [x] rate limiter/message size cap

## Simulation

- [x] seeded map generator
- [x] 20 Hz fixed-step active round loop
- [x] loop stopped outside `playing`
- [x] input freshness timeout
- [x] movement/AABB collision
- [x] soil digging
- [x] treasure placement/pickup
- [x] 10 Hz snapshot cap
- [x] client prediction/reconciliation
- [x] remote interpolation
- [x] 2–5 sec recoverable checkpoint

## Classic loop

- [x] shop before round 1
- [x] cash persists across rounds
- [x] Small Charge
- [x] explosion/chain reaction
- [x] soil destruction
- [x] damage/death
- [x] round end conditions
- [x] result -> shop
- [x] 5-round match result
- [x] tie rules
- [x] extra equipment (Heavy Charge, Proximity Mine, Pickaxe II, Med Kit)

## Assets

- [x] placeholder tiles/player/bomb/explosion/pickup render correctly
- [x] 32x32 nearest-neighbor contract
- [x] miner 4 directions x 4 frames
- [x] terrain set
- [x] explosion segments
- [x] shop/equipment icons
- [x] asset manifest complete

## Deployment

- [x] Client production build verified
- [x] GitHub Pages Action green configuration
- [x] Dual-mode client: full multiplayer network + instant solo practice mode vs bots for live web demo
- [x] Production Origin configuration
