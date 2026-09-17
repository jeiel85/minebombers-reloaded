import {
  BASE_SOIL_DURABILITY,
  DEFAULT_ROUND_DURATION_MS,
  INPUT_STALE_MS,
  KILL_BOUNTY,
  MAP_HEIGHT,
  MAP_WIDTH,
  MAX_HP,
  SPAWN_INVULNERABILITY_MS,
  SURVIVOR_BONUS,
  WORLD_UNITS_PER_TILE,
} from './constants';
import { aabbIntersects, getPlayerAABB, getTileAABB, tileIndexOf } from './collision';
import { movePlayerWithCollision } from './movement';
import { stepDigging } from './digging';
import { computeExplosion } from './explosions';
import { EQUIPMENT, type EquipmentId } from './equipment';
import { calculateRoundStandings } from './economy';
import type {
  SimExplosive,
  SimFallingRock,
  SimMatchStandingsEntry,
  SimMine,
  SimMonster,
  SimPickupItem,
  SimPlayer,
  SimProjectile,
  SimRoundStandingsEntry,
  SimTileState,
  SimTreasureEntity,
  SimWorldCheckpoint,
} from './types';
import type {
  EntityView,
  GameEvent,
  InventoryState,
  RoomPhase,
  ServerMessage,
  TileDelta,
  TileKind,
} from '../protocol';
import type { GeneratedMap } from './mapGenerator';

export const HOTBAR_SLOTS = [
  'small_charge',
  'dynamite',
  'heavy_charge',
  'remote_bomb',
  'proximity_mine',
  'rocket',
  'flamethrower',
  'nuke',
] as const;

export interface ActionQueueItem {
  playerId: string;
  action: 'place_bomb' | 'use_secondary' | 'interact' | 'use_item';
  slot?: number;
  seq: number;
}

export class WorldSimulation {
  public roundIndex = 1;
  public simTime = 0; // simulation time in ms
  public roundStartsAt = 0;
  public roundEndsAt = 0;
  public phase: RoomPhase = 'playing';
  public serverSeq = 0;

  public tiles: SimTileState[] = [];
  public players: SimPlayer[] = [];
  public explosives: SimExplosive[] = [];
  public mines: SimMine[] = [];
  public treasures: SimTreasureEntity[] = [];
  public pickups: SimPickupItem[] = [];
  public projectiles: SimProjectile[] = [];
  public monsters: SimMonster[] = [];
  public fallingRocks: SimFallingRock[] = [];
  public isCollapsing = false;

  public pendingActions: ActionQueueItem[] = [];
  public recentEvents: GameEvent[] = [];
  public changedTiles: Map<number, TileDelta> = new Map();

  private explosiveIdCounter = 0;
  private projectileIdCounter = 0;
  private monsterIdCounter = 0;
  private collapseTimer = 0;

  constructor(
    public readonly seed: number,
    mapData: GeneratedMap,
    initialPlayers: Array<{ id: string; name: string; cash: number; inventory: InventoryState; stats?: SimPlayer['stats'] }>,
    roundIndex = 1,
    roundDurationMs = DEFAULT_ROUND_DURATION_MS,
  ) {
    this.roundIndex = roundIndex;
    this.roundStartsAt = 0;
    this.roundEndsAt = roundDurationMs;

    // Initialize tiles from generated map
    this.tiles = mapData.tiles.map((kind) => ({
      kind,
      durability: kind === 'soil' ? BASE_SOIL_DURABILITY : 0,
    }));

    // Initialize treasures
    this.treasures = mapData.treasures.map((t) => {
      const tileX = t.index % MAP_WIDTH;
      const tileY = Math.floor(t.index / MAP_WIDTH);
      return {
        id: t.id,
        tileIndex: t.index,
        tileX,
        tileY,
        rarity: t.rarity,
        value: t.value,
        revealed: false,
        collected: false,
      };
    });

    // Initialize players at spawn candidates
    this.players = initialPlayers.map((p, idx) => {
      const spawn = mapData.spawns[idx] ?? { x: 2, y: 2 };
      return {
        id: p.id,
        name: p.name,
        x: (spawn.x + 0.5) * WORLD_UNITS_PER_TILE,
        y: (spawn.y + 0.5) * WORLD_UNITS_PER_TILE,
        hp: MAX_HP,
        alive: true,
        cash: p.cash,
        inventory: JSON.parse(JSON.stringify(p.inventory)) as InventoryState,
        input: {
          dx: 0,
          dy: 0,
          seq: 0,
          clientTime: 0,
          lastReceivedAt: 0,
        },
        invulnerableUntil: SPAWN_INVULNERABILITY_MS,
        stats: p.stats ? { ...p.stats } : { kills: 0, treasureValue: 0, roundWins: 0 },
        digTargetTile: null,
      };
    });

    // Spawn cavern monsters in open mine areas
    const openFloorIndices: number[] = [];
    for (let i = 0; i < this.tiles.length; i++) {
      if (this.tiles[i]?.kind === 'floor') {
        const tx = i % MAP_WIDTH;
        const ty = Math.floor(i / MAP_WIDTH);
        if (tx > 4 && tx < MAP_WIDTH - 5 && ty > 4 && ty < MAP_HEIGHT - 5) {
          openFloorIndices.push(i);
        }
      }
    }
    const monsterCount = Math.min(3, openFloorIndices.length);
    for (let m = 0; m < monsterCount; m++) {
      const idx = openFloorIndices[(m * 7 + seed) % openFloorIndices.length]!;
      const mx = idx % MAP_WIDTH;
      const my = Math.floor(idx / MAP_WIDTH);
      this.monsters.push({
        id: `monster_${++this.monsterIdCounter}`,
        kind: m % 2 === 0 ? 'slime' : 'bat',
        x: (mx + 0.5) * WORLD_UNITS_PER_TILE,
        y: (my + 0.5) * WORLD_UNITS_PER_TILE,
        hp: 35,
        maxHp: 35,
        moveTimerMs: 0,
        dx: 1,
        dy: 0,
        alive: true,
        bounty: 100,
      });
    }
  }

  public setPlayerInput(
    playerId: string,
    dx: -1 | 0 | 1,
    dy: -1 | 0 | 1,
    seq: number,
    clientTime: number,
    now: number,
    primary = false,
    secondary = false,
    slot?: number,
  ): void {
    const p = this.players.find((pl) => pl.id === playerId);
    if (!p || !p.alive) return;

    if (slot !== undefined && slot >= 0 && slot < 8) {
      p.inventory.selectedSlot = slot;
    }

    p.input = {
      dx,
      dy,
      seq,
      clientTime,
      lastReceivedAt: now,
      primary,
      secondary,
    };

    if (primary) {
      this.pendingActions.push({ playerId, action: 'place_bomb', slot: p.inventory.selectedSlot, seq });
    }
    if (secondary) {
      this.pendingActions.push({ playerId, action: 'use_secondary', slot: p.inventory.selectedSlot, seq });
    }
  }

  public queueAction(playerId: string, action: ActionQueueItem['action'], slot = 0, seq: number): void {
    const p = this.players.find((pl) => pl.id === playerId);
    if (!p || !p.alive) return;
    this.pendingActions.push({ playerId, action, slot, seq });
  }

  /**
   * Deterministic 13-step world update per SIM_STEP_MS (50ms).
   */
  public step(stepMs = 50): GameEvent[] {
    if (this.phase !== 'playing') return [];

    this.serverSeq++;
    this.simTime += stepMs;
    const stepEvents: GameEvent[] = [];

    // 1. Expire/neutralize invalid or stale inputs (> 1000 ms)
    for (const player of this.players) {
      if (!player.alive) continue;
      if (this.simTime - player.input.lastReceivedAt > INPUT_STALE_MS) {
        player.input.dx = 0;
        player.input.dy = 0;
      }
    }

    // 2 & 3. Apply movement intent & resolve AABB collisions / sliding
    for (const player of this.players) {
      if (!player.alive) continue;
      const res = movePlayerWithCollision(
        player.x,
        player.y,
        player.input.dx,
        player.input.dy,
        stepMs,
        this.tiles,
      );
      player.x = res.x;
      player.y = res.y;

      // 4. Accumulate and resolve digging
      const digRes = stepDigging(
        player,
        res.contactedSoilTile,
        stepMs,
        this.tiles,
        this.treasures,
      );

      if (digRes) {
        this.changedTiles.set(digRes.tileIndex, {
          index: digRes.tileIndex,
          kind: this.tiles[digRes.tileIndex]?.kind ?? 'floor',
          digProgressPermille: digRes.digProgressPermille,
        });

        if (digRes.tileOpened) {
          stepEvents.push({
            kind: 'tile_changed',
            index: digRes.tileIndex,
            tile: 'floor',
          });

          // If no treasure on this tile, 20% chance to drop ammo or med kit
          if (!digRes.revealedTreasure) {
            const rand = Math.random();
            if (rand < 0.20) {
              const defId: 'ammo' | 'med_kit' = rand < 0.13 ? 'ammo' : 'med_kit';
              this.pickups.push({
                id: `pickup_${digRes.tileIndex}`,
                tileX: digRes.tileIndex % MAP_WIDTH,
                tileY: Math.floor(digRes.tileIndex / MAP_WIDTH),
                definitionId: defId,
                collected: false,
              });
            }
          }
        }
      }
    }

    // 5. Process discrete actions
    while (this.pendingActions.length > 0) {
      const act = this.pendingActions.shift()!;
      const player = this.players.find((p) => p.id === act.playerId);
      if (!player || !player.alive) continue;

      if (act.slot !== undefined) {
        player.inventory.selectedSlot = act.slot;
      }

      if (act.action === 'place_bomb') {
        this.handlePlaceBomb(player, stepEvents);
      } else if (act.action === 'use_item' || act.action === 'use_secondary') {
        this.handleUseItem(player, stepEvents);
      }
    }

    // 6. Proximity Mine triggers
    for (const mine of this.mines) {
      if (mine.detonated) continue;
      if (this.simTime < mine.armedAt) continue;

      const mineCenter = {
        x: (mine.tileX + 0.5) * WORLD_UNITS_PER_TILE,
        y: (mine.tileY + 0.5) * WORLD_UNITS_PER_TILE,
      };

      for (const p of this.players) {
        if (!p.alive) continue;
        const distSq = (p.x - mineCenter.x) ** 2 + (p.y - mineCenter.y) ** 2;
        if (distSq <= mine.triggerRadiusUnits ** 2) {
          mine.detonated = true;
          break;
        }
      }
    }

    // 7. Resolve due explosives & detonated mines in stable ID order
    const dueExplosives = this.explosives
      .filter((e) => e.explodeAt <= this.simTime)
      .sort((a, b) => a.id.localeCompare(b.id));

    const dueMines = this.mines
      .filter((m) => m.detonated)
      .sort((a, b) => a.id.localeCompare(b.id));

    // Remove detonating entities from state
    if (dueExplosives.length > 0) {
      const explodingIds = new Set(dueExplosives.map((e) => e.id));
      this.explosives = this.explosives.filter((e) => !explodingIds.has(e.id));
    }
    if (dueMines.length > 0) {
      const explodingMineIds = new Set(dueMines.map((m) => m.id));
      this.mines = this.mines.filter((m) => !explodingMineIds.has(m.id));
    }

    // Execute detonations
    for (const exp of dueExplosives) {
      const expRes = computeExplosion(
        exp.id,
        exp.ownerId,
        exp.tileX,
        exp.tileY,
        exp.radius,
        exp.damage,
        exp.penetration,
        this.simTime,
        this.tiles,
        this.players,
        this.explosives,
        this.mines,
        this.treasures,
      );
      for (const idx of expRes.openedTiles) {
        this.changedTiles.set(idx, {
          index: idx,
          kind: 'floor',
          digProgressPermille: 1000,
        });
      }
      stepEvents.push(...expRes.events);
    }

    for (const mine of dueMines) {
      const mineRes = computeExplosion(
        mine.id,
        mine.ownerId,
        mine.tileX,
        mine.tileY,
        mine.radius,
        mine.damage,
        0,
        this.simTime,
        this.tiles,
        this.players,
        this.explosives,
        this.mines,
        this.treasures,
      );
      for (const idx of mineRes.openedTiles) {
        this.changedTiles.set(idx, {
          index: idx,
          kind: 'floor',
          digProgressPermille: 1000,
        });
      }
      stepEvents.push(...mineRes.events);
    }

    // 10b. Step Projectiles (Rockets)
    const dt = stepMs / 1000;
    for (const proj of this.projectiles) {
      if (!proj.active) continue;
      proj.x += proj.vx * dt;
      proj.y += proj.vy * dt;

      const tX = Math.floor(proj.x / WORLD_UNITS_PER_TILE);
      const tY = Math.floor(proj.y / WORLD_UNITS_PER_TILE);
      let collided = false;

      // 1. Boundary & Solid tiles
      if (tX <= 0 || tX >= MAP_WIDTH - 1 || tY <= 0 || tY >= MAP_HEIGHT - 1) {
        collided = true;
      } else {
        const tIdx = tY * MAP_WIDTH + tX;
        const tile = this.tiles[tIdx];
        if (tile && (tile.kind === 'rock' || tile.kind === 'soil')) {
          collided = true;
        }
      }

      // 2. Players
      if (!collided) {
        const projBox = { minX: proj.x - 250, minY: proj.y - 250, maxX: proj.x + 250, maxY: proj.y + 250 };
        for (const p of this.players) {
          if (!p.alive || p.id === proj.ownerId) continue;
          if (aabbIntersects(projBox, getPlayerAABB(p.x, p.y))) {
            collided = true;
            break;
          }
        }
      }

      // 3. Monsters
      if (!collided) {
        const projBox = { minX: proj.x - 250, minY: proj.y - 250, maxX: proj.x + 250, maxY: proj.y + 250 };
        for (const m of this.monsters) {
          if (!m.alive) continue;
          const mBox = { minX: m.x - 256, minY: m.y - 256, maxX: m.x + 256, maxY: m.y + 256 };
          if (aabbIntersects(projBox, mBox)) {
            collided = true;
            break;
          }
        }
      }

      if (collided) {
        proj.active = false;
        const clampedX = Math.max(1, Math.min(MAP_WIDTH - 2, tX));
        const clampedY = Math.max(1, Math.min(MAP_HEIGHT - 2, tY));
        const expRes = computeExplosion(
          proj.id,
          proj.ownerId,
          clampedX,
          clampedY,
          proj.blastRadiusTiles,
          proj.damage,
          0,
          this.simTime,
          this.tiles,
          this.players,
          this.explosives,
          this.mines,
          this.treasures,
        );
        for (const idx of expRes.openedTiles) {
          this.changedTiles.set(idx, { index: idx, kind: 'floor', digProgressPermille: 1000 });
        }
        stepEvents.push(...expRes.events);
      }
    }
    this.projectiles = this.projectiles.filter((p) => p.active);

    // 10c. Step Monsters (Patrol, Contact Damage, and Elimination)
    for (const m of this.monsters) {
      if (!m.alive) continue;
      m.moveTimerMs -= stepMs;
      if (m.moveTimerMs <= 0) {
        m.moveTimerMs = 800 + Math.floor(Math.random() * 800);
        const dirs: Array<[-1 | 0 | 1, -1 | 0 | 1]> = [[0, -1], [0, 1], [-1, 0], [1, 0]];
        const d = dirs[Math.floor(Math.random() * dirs.length)]!;
        m.dx = d[0];
        m.dy = d[1];
      }

      const speed = m.kind === 'bat' ? 2200 : 1400;
      const nextX = m.x + m.dx * speed * dt;
      const nextY = m.y + m.dy * speed * dt;
      const ntX = Math.floor(nextX / WORLD_UNITS_PER_TILE);
      const ntY = Math.floor(nextY / WORLD_UNITS_PER_TILE);
      const nIdx = ntY * MAP_WIDTH + ntX;
      const nTile = this.tiles[nIdx];

      if (nTile && nTile.kind === 'floor') {
        m.x = nextX;
        m.y = nextY;
      } else {
        m.dx = -m.dx as -1 | 0 | 1;
        m.dy = -m.dy as -1 | 0 | 1;
        m.moveTimerMs = 0;
      }

      // Check contact damage with living players
      const mBox = { minX: m.x - 300, minY: m.y - 300, maxX: m.x + 300, maxY: m.y + 300 };
      for (const p of this.players) {
        if (!p.alive || this.simTime < p.invulnerableUntil) continue;
        if (aabbIntersects(mBox, getPlayerAABB(p.x, p.y))) {
          let dmg = 15;
          if (p.inventory.upgrades['kevlar_armor'] || p.inventory.items['kevlar_armor']) {
            dmg = 9;
          }
          p.hp = Math.max(0, p.hp - dmg);
          stepEvents.push({
            kind: 'damage',
            playerId: p.id,
            sourceId: m.kind,
            amount: dmg,
            hpAfter: p.hp,
          });
          if (p.hp <= 0) {
            p.alive = false;
            stepEvents.push({
              kind: 'player_eliminated',
              playerId: p.id,
              killerPlayerId: null,
            });
          }
        }
      }
    }

    // 10d. Sudden Death: Mine Collapse (when <= 45s remaining)
    const remainingMs = Math.max(0, this.roundEndsAt - this.simTime);
    if (remainingMs <= 45_000 && remainingMs > 0) {
      if (!this.isCollapsing) {
        this.isCollapsing = true;
        stepEvents.push({ kind: 'mine_collapse', fallingRocks: [] });
      }

      this.collapseTimer += stepMs;
      if (this.collapseTimer >= 2200) {
        this.collapseTimer = 0;
        const rocks: Array<{ tileX: number; tileY: number }> = [];
        for (let r = 0; r < 2; r++) {
          const rx = 2 + Math.floor(Math.random() * (MAP_WIDTH - 4));
          const ry = 2 + Math.floor(Math.random() * (MAP_HEIGHT - 4));
          rocks.push({ tileX: rx, tileY: ry });
          const tBox = getTileAABB(rx, ry);
          for (const p of this.players) {
            if (!p.alive) continue;
            if (aabbIntersects(getPlayerAABB(p.x, p.y), tBox)) {
              p.hp = Math.max(0, p.hp - 35);
              stepEvents.push({
                kind: 'damage',
                playerId: p.id,
                sourceId: 'falling_rock',
                amount: 35,
                hpAfter: p.hp,
              });
              if (p.hp <= 0) {
                p.alive = false;
                stepEvents.push({
                  kind: 'player_eliminated',
                  playerId: p.id,
                  killerPlayerId: null,
                });
              }
            }
          }
        }
        stepEvents.push({ kind: 'mine_collapse', fallingRocks: rocks });
      }
    }

    // 11. Pickups / Treasure collection
    for (const treasure of this.treasures) {
      if (!treasure.revealed || treasure.collected) continue;

      const tBox = getTileAABB(treasure.tileX, treasure.tileY);
      for (const player of this.players) {
        if (!player.alive) continue;
        const pBox = getPlayerAABB(player.x, player.y);
        if (aabbIntersects(pBox, tBox)) {
          treasure.collected = true;
          player.cash += treasure.value;
          player.stats.treasureValue += treasure.value;

          stepEvents.push({
            kind: 'treasure_collected',
            playerId: player.id,
            entityId: treasure.id,
            value: treasure.value,
          });
          stepEvents.push({
            kind: 'cash_changed',
            playerId: player.id,
            cash: player.cash,
            reason: 'treasure',
          });
          break;
        }
      }
    }

    for (const pickup of this.pickups) {
      if (pickup.collected) continue;
      const tBox = getTileAABB(pickup.tileX, pickup.tileY);
      for (const player of this.players) {
        if (!player.alive) continue;
        const pBox = getPlayerAABB(player.x, player.y);
        if (aabbIntersects(pBox, tBox)) {
          pickup.collected = true;
          if (pickup.definitionId === 'ammo') {
            player.inventory.items['small_charge'] = (player.inventory.items['small_charge'] ?? 0) + 2;
            stepEvents.push({
              kind: 'pickup_collected',
              playerId: player.id,
              entityId: pickup.id,
              definitionId: 'ammo',
            });
          } else if (pickup.definitionId === 'med_kit') {
            player.inventory.items['med_kit'] = (player.inventory.items['med_kit'] ?? 0) + 1;
            stepEvents.push({
              kind: 'pickup_collected',
              playerId: player.id,
              entityId: pickup.id,
              definitionId: 'med_kit',
            });
          }
          break;
        }
      }
    }

    // 12. Evaluate round-end conditions
    const alivePlayers = this.players.filter((p) => p.alive);
    const uncollectedTreasures = this.treasures.filter((t) => !t.collected);

    let roundEndReason: 'timeout' | 'treasure_empty' | 'last_survivor' | null = null;
    if (this.simTime >= this.roundEndsAt) {
      roundEndReason = 'timeout';
    } else if (uncollectedTreasures.length === 0) {
      roundEndReason = 'treasure_empty';
    } else if (this.players.length >= 2 && alivePlayers.length < 2) {
      roundEndReason = 'last_survivor';
    }

    if (roundEndReason) {
      this.phase = 'round_result';
      // Survivor bonus if exactly 1 survivor
      if (alivePlayers.length === 1) {
        const survivor = alivePlayers[0]!;
        survivor.cash += SURVIVOR_BONUS;
        survivor.stats.roundWins += 1;
        stepEvents.push({
          kind: 'cash_changed',
          playerId: survivor.id,
          cash: survivor.cash,
          reason: 'survivor_bonus',
        });
      }

      stepEvents.push({
        kind: 'round_ended',
        roundIndex: this.roundIndex,
        reason: roundEndReason,
      });
      stepEvents.push({
        kind: 'phase_changed',
        phase: 'round_result',
        at: this.simTime,
      });
    }

    this.recentEvents.push(...stepEvents);
    return stepEvents;
  }

  private handlePlaceBomb(player: SimPlayer, stepEvents: GameEvent[]): void {
    const tileX = Math.max(1, Math.min(MAP_WIDTH - 2, Math.floor(player.x / WORLD_UNITS_PER_TILE)));
    const tileY = Math.max(1, Math.min(MAP_HEIGHT - 2, Math.floor(player.y / WORLD_UNITS_PER_TILE)));

    // Select weapon according to selectedSlot or first available weapon
    const slotIdx = player.inventory.selectedSlot ?? 0;
    let targetEquipId: string = HOTBAR_SLOTS[slotIdx] ?? 'small_charge';
    if ((player.inventory.items[targetEquipId] ?? 0) <= 0) {
      const available = HOTBAR_SLOTS.find((s) => (player.inventory.items[s] ?? 0) > 0);
      if (available) {
        targetEquipId = available;
      } else {
        targetEquipId = 'pickaxe_1';
      }
    }

    if (targetEquipId === 'rocket') {
      player.inventory.items['rocket'] = (player.inventory.items['rocket'] ?? 1) - 1;
      const id = `proj_${++this.projectileIdCounter}`;
      let vx = 0;
      let vy = 6000;
      if (player.input.dx !== 0 || player.input.dy !== 0) {
        vx = player.input.dx * 6000;
        vy = player.input.dy * 6000;
      }
      this.projectiles.push({
        id,
        ownerId: player.id,
        definitionId: 'rocket',
        x: player.x,
        y: player.y,
        vx,
        vy,
        damage: EQUIPMENT.rocket.damage ?? 85,
        blastRadiusTiles: EQUIPMENT.rocket.blastRadiusTiles ?? 2,
        active: true,
      });
      stepEvents.push({
        kind: 'projectile_fired',
        id,
        ownerId: player.id,
        x: player.x,
        y: player.y,
        vx,
        vy,
      });
      return;
    }

    if (targetEquipId === 'flamethrower') {
      player.inventory.items['flamethrower'] = (player.inventory.items['flamethrower'] ?? 1) - 1;
      const fdx = player.input.dx !== 0 ? player.input.dx : (player.input.dy === 0 ? 0 : 0);
      const fdy = player.input.dy !== 0 ? player.input.dy : (fdx === 0 ? 1 : 0);
      const flameCells: Array<{ x: number; y: number }> = [];

      for (let dist = 1; dist <= 3; dist++) {
        const cx = tileX + fdx * dist;
        const cy = tileY + fdy * dist;
        if (cx <= 0 || cx >= MAP_WIDTH - 1 || cy <= 0 || cy >= MAP_HEIGHT - 1) break;
        const idx = cy * MAP_WIDTH + cx;
        const tile = this.tiles[idx];
        if (!tile || tile.kind === 'rock') break;

        flameCells.push({ x: cx, y: cy });
        if (tile.kind === 'soil') {
          tile.kind = 'floor';
          tile.durability = 0;
          this.changedTiles.set(idx, { index: idx, kind: 'floor', digProgressPermille: 1000 });
          stepEvents.push({ kind: 'tile_changed', index: idx, tile: 'floor' });
        }
      }

      for (const p of this.players) {
        if (p.id === player.id || !p.alive || this.simTime < p.invulnerableUntil) continue;
        const pBox = getPlayerAABB(p.x, p.y);
        for (const cell of flameCells) {
          if (aabbIntersects(pBox, getTileAABB(cell.x, cell.y))) {
            let dmg: number = EQUIPMENT.flamethrower.damage ?? 40;
            if (p.inventory.upgrades['kevlar_armor'] || p.inventory.items['kevlar_armor']) {
              dmg = Math.round(dmg * 0.6);
            }
            p.hp = Math.max(0, p.hp - dmg);
            stepEvents.push({ kind: 'damage', playerId: p.id, sourceId: 'flamethrower', amount: dmg, hpAfter: p.hp });
            if (p.hp <= 0) {
              p.alive = false;
              player.cash += KILL_BOUNTY;
              player.stats.kills += 1;
              stepEvents.push({ kind: 'player_eliminated', playerId: p.id, killerPlayerId: player.id });
              stepEvents.push({ kind: 'cash_changed', playerId: player.id, cash: player.cash, reason: 'bounty' });
            }
            break;
          }
        }
      }

      stepEvents.push({ kind: 'flame_burst', playerId: player.id, cells: flameCells });
      return;
    }

    if (targetEquipId === 'remote_bomb') {
      const currentRemote = this.explosives.filter((e) => e.ownerId === player.id && e.isRemote).length;
      if (currentRemote >= (EQUIPMENT.remote_bomb.maxActive ?? 3)) return;

      player.inventory.items['remote_bomb'] = (player.inventory.items['remote_bomb'] ?? 1) - 1;
      const id = `exp_${++this.explosiveIdCounter}`;
      this.explosives.push({
        id,
        ownerId: player.id,
        definitionId: 'remote_bomb',
        tileX,
        tileY,
        explodeAt: Infinity,
        radius: EQUIPMENT.remote_bomb.blastRadiusTiles ?? 3,
        damage: EQUIPMENT.remote_bomb.damage ?? 95,
        penetration: 0,
        isRemote: true,
      });
      stepEvents.push({
        kind: 'explosive_placed',
        id,
        ownerId: player.id,
        definitionId: 'remote_bomb',
        tileX,
        tileY,
        explodeAt: 0,
      });
      return;
    }

    if (targetEquipId === 'proximity_mine') {
      const activeCount = this.mines.filter((m) => m.ownerId === player.id).length;
      if (activeCount >= (EQUIPMENT.proximity_mine.maxActive ?? 3)) return;

      player.inventory.items['proximity_mine'] = (player.inventory.items['proximity_mine'] ?? 1) - 1;
      const id = `mine_${++this.explosiveIdCounter}`;
      const armedAt = this.simTime + (EQUIPMENT.proximity_mine.cooldownMs ?? 800);
      this.mines.push({
        id,
        ownerId: player.id,
        definitionId: 'proximity_mine',
        tileX,
        tileY,
        armedAt,
        triggerRadiusUnits: 768,
        radius: EQUIPMENT.proximity_mine.blastRadiusTiles ?? 2,
        damage: EQUIPMENT.proximity_mine.damage ?? 100,
      });
      stepEvents.push({
        kind: 'explosive_placed',
        id,
        ownerId: player.id,
        definitionId: 'proximity_mine',
        tileX,
        tileY,
        explodeAt: armedAt,
      });
      return;
    }

    if (targetEquipId === 'nuke') {
      player.inventory.items['nuke'] = (player.inventory.items['nuke'] ?? 1) - 1;
      const id = `exp_${++this.explosiveIdCounter}`;
      const explodeAt = this.simTime + (EQUIPMENT.nuke.fuseMs ?? 3500);
      this.explosives.push({
        id,
        ownerId: player.id,
        definitionId: 'nuke',
        tileX,
        tileY,
        explodeAt,
        radius: EQUIPMENT.nuke.blastRadiusTiles ?? 8,
        damage: EQUIPMENT.nuke.damage ?? 200,
        penetration: 3,
      });
      stepEvents.push({
        kind: 'explosive_placed',
        id,
        ownerId: player.id,
        definitionId: 'nuke',
        tileX,
        tileY,
        explodeAt,
      });
      return;
    }

    if (targetEquipId === 'dynamite') {
      player.inventory.items['dynamite'] = (player.inventory.items['dynamite'] ?? 1) - 1;
      const id = `exp_${++this.explosiveIdCounter}`;
      const explodeAt = this.simTime + (EQUIPMENT.dynamite.fuseMs ?? 2000);
      this.explosives.push({
        id,
        ownerId: player.id,
        definitionId: 'dynamite',
        tileX,
        tileY,
        explodeAt,
        radius: EQUIPMENT.dynamite.blastRadiusTiles ?? 3,
        damage: EQUIPMENT.dynamite.damage ?? 85,
        penetration: 0,
      });
      stepEvents.push({
        kind: 'explosive_placed',
        id,
        ownerId: player.id,
        definitionId: 'dynamite',
        tileX,
        tileY,
        explodeAt,
      });
      return;
    }

    if (targetEquipId === 'heavy_charge') {
      player.inventory.items['heavy_charge'] = (player.inventory.items['heavy_charge'] ?? 1) - 1;
      const id = `exp_${++this.explosiveIdCounter}`;
      const explodeAt = this.simTime + (EQUIPMENT.heavy_charge.fuseMs ?? 2400);
      this.explosives.push({
        id,
        ownerId: player.id,
        definitionId: 'heavy_charge',
        tileX,
        tileY,
        explodeAt,
        radius: EQUIPMENT.heavy_charge.blastRadiusTiles ?? 4,
        damage: EQUIPMENT.heavy_charge.damage ?? 100,
        penetration: 1,
      });
      stepEvents.push({
        kind: 'explosive_placed',
        id,
        ownerId: player.id,
        definitionId: 'heavy_charge',
        tileX,
        tileY,
        explodeAt,
      });
      return;
    }

    if (targetEquipId === 'small_charge') {
      const activeCount = this.explosives.filter((e) => e.ownerId === player.id && !e.isRemote).length;
      if (activeCount >= (EQUIPMENT.small_charge.maxActive ?? 3)) return;

      player.inventory.items['small_charge'] = (player.inventory.items['small_charge'] ?? 1) - 1;
      const id = `exp_${++this.explosiveIdCounter}`;
      const explodeAt = this.simTime + (EQUIPMENT.small_charge.fuseMs ?? 1600);
      this.explosives.push({
        id,
        ownerId: player.id,
        definitionId: 'small_charge',
        tileX,
        tileY,
        explodeAt,
        radius: EQUIPMENT.small_charge.blastRadiusTiles ?? 2,
        damage: EQUIPMENT.small_charge.damage ?? 65,
        penetration: 0,
      });
      stepEvents.push({
        kind: 'explosive_placed',
        id,
        ownerId: player.id,
        definitionId: 'small_charge',
        tileX,
        tileY,
        explodeAt,
      });
      return;
    }

    // Pickaxe Melee Attack (default fallback when out of weapons)
    let target: SimPlayer | SimMonster | null = null;
    let minDistance = 1024; // 1 tile range

    for (const opp of this.players) {
      if (opp.id === player.id || !opp.alive) continue;
      const dist = Math.hypot(opp.x - player.x, opp.y - player.y);
      if (dist <= minDistance) {
        minDistance = dist;
        target = opp;
      }
    }

    if (!target) {
      for (const m of this.monsters) {
        if (!m.alive) continue;
        const dist = Math.hypot(m.x - player.x, m.y - player.y);
        if (dist <= minDistance) {
          minDistance = dist;
          target = m;
        }
      }
    }

    if (target) {
      const damage = 25;
      target.hp = Math.max(0, target.hp - damage);
      const eliminated = target.hp <= 0;
      if (eliminated) target.alive = false;

      stepEvents.push({
        kind: 'damage',
        playerId: target.id,
        sourceId: 'pickaxe',
        amount: damage,
        hpAfter: target.hp,
      });

      if (eliminated) {
        const bounty = 'bounty' in target ? target.bounty : KILL_BOUNTY;
        player.cash += bounty;
        player.stats.kills += 1;
        stepEvents.push({
          kind: 'player_eliminated',
          playerId: target.id,
          killerPlayerId: player.id,
        });
        stepEvents.push({
          kind: 'cash_changed',
          playerId: player.id,
          cash: player.cash,
          reason: 'bounty',
        });
      }
    }
  }

  private handleUseItem(player: SimPlayer, stepEvents: GameEvent[]): void {
    // 1. Detonate remote bombs if owned on the field
    const ownedRemotes = this.explosives.filter((e) => e.ownerId === player.id && e.isRemote);
    if (ownedRemotes.length > 0) {
      for (const r of ownedRemotes) {
        r.explodeAt = this.simTime;
      }
      return;
    }

    // 2. Teleport consumable
    const teleports = player.inventory.items['teleport'] ?? 0;
    if (teleports > 0) {
      player.inventory.items['teleport'] = teleports - 1;
      const safeFloorIndices: number[] = [];
      for (let i = 0; i < this.tiles.length; i++) {
        if (this.tiles[i]?.kind === 'floor') {
          safeFloorIndices.push(i);
        }
      }
      if (safeFloorIndices.length > 0) {
        const targetIdx = safeFloorIndices[Math.floor(Math.random() * safeFloorIndices.length)]!;
        const tx = targetIdx % MAP_WIDTH;
        const ty = Math.floor(targetIdx / MAP_WIDTH);
        player.x = (tx + 0.5) * WORLD_UNITS_PER_TILE;
        player.y = (ty + 0.5) * WORLD_UNITS_PER_TILE;
        stepEvents.push({
          kind: 'teleported',
          playerId: player.id,
          tileX: tx,
          tileY: ty,
        });
      }
      return;
    }

    // 3. Med Kit
    const medKits = player.inventory.items['med_kit'] ?? 0;
    if (medKits > 0 && player.hp < MAX_HP) {
      player.inventory.items['med_kit'] = medKits - 1;
      player.hp = Math.min(MAX_HP, player.hp + (EQUIPMENT.med_kit.heal ?? 50));
      stepEvents.push({
        kind: 'damage',
        playerId: player.id,
        sourceId: 'med_kit',
        amount: -50,
        hpAfter: player.hp,
      });
    }
  }

  public getSnapshot(forPlayerId?: string): Extract<ServerMessage, { t: 's.snapshot' }> {
    const ackPlayer = this.players.find((p) => p.id === forPlayerId);
    const ackInputSeq = ackPlayer?.input.seq ?? 0;

    const entities: EntityView[] = [
      ...this.explosives.map((e): EntityView => ({
        kind: 'explosive',
        id: e.id,
        ownerId: e.ownerId,
        definitionId: e.definitionId,
        tileX: e.tileX,
        tileY: e.tileY,
        explodeAt: e.explodeAt,
      })),
      ...this.mines.map((m): EntityView => ({
        kind: 'explosive',
        id: m.id,
        ownerId: m.ownerId,
        definitionId: m.definitionId,
        tileX: m.tileX,
        tileY: m.tileY,
        explodeAt: m.armedAt,
      })),
      ...this.treasures
        .filter((t) => t.revealed && !t.collected)
        .map((t): EntityView => ({
          kind: 'treasure',
          id: t.id,
          tileX: t.tileX,
          tileY: t.tileY,
          value: t.value,
          rarity: t.rarity,
        })),
      ...this.pickups
        .filter((p) => !p.collected)
        .map((p): EntityView => ({
          kind: 'pickup',
          id: p.id,
          tileX: p.tileX,
          tileY: p.tileY,
          definitionId: p.definitionId,
        })),
      ...this.projectiles
        .filter((p) => p.active)
        .map((p): EntityView => ({
          kind: 'projectile',
          id: p.id,
          definitionId: p.definitionId,
          x: p.x,
          y: p.y,
          vx: p.vx,
          vy: p.vy,
        })),
      ...this.monsters
        .filter((m) => m.alive)
        .map((m): EntityView => ({
          kind: 'monster',
          id: m.id,
          monsterKind: m.kind,
          x: m.x,
          y: m.y,
          hp: m.hp,
        })),
    ];

    const changedTiles = Array.from(this.changedTiles.values());
    // Clear delta buffer after extracting
    this.changedTiles.clear();

    return {
      t: 's.snapshot',
      serverSeq: this.serverSeq,
      serverTime: this.simTime,
      ackInputSeq,
      phase: this.phase,
      roundIndex: this.roundIndex,
      roundEndsAt: this.roundEndsAt,
      players: this.players.map((p) => ({
        id: p.id,
        x: p.x,
        y: p.y,
        hp: p.hp,
        alive: p.alive,
        cash: p.cash,
      })),
      entities,
      changedTiles,
    };
  }

  public getRoundStandings(): SimRoundStandingsEntry[] {
    return calculateRoundStandings(this.players);
  }

  public createCheckpoint(): SimWorldCheckpoint {
    return {
      roundIndex: this.roundIndex,
      simTime: this.simTime,
      roundEndsAt: this.roundEndsAt,
      seed: this.seed,
      players: JSON.parse(JSON.stringify(this.players)),
      explosives: JSON.parse(JSON.stringify(this.explosives)),
      mines: JSON.parse(JSON.stringify(this.mines)),
      treasures: JSON.parse(JSON.stringify(this.treasures)),
      tiles: JSON.parse(JSON.stringify(this.tiles)),
    };
  }
}
