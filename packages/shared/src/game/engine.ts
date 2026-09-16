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
  SimMatchStandingsEntry,
  SimMine,
  SimPickupItem,
  SimPlayer,
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

  public pendingActions: ActionQueueItem[] = [];
  public recentEvents: GameEvent[] = [];
  public changedTiles: Map<number, TileDelta> = new Map();

  private explosiveIdCounter = 0;

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
  ): void {
    const p = this.players.find((pl) => pl.id === playerId);
    if (!p || !p.alive) return;

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
      this.pendingActions.push({ playerId, action: 'place_bomb', seq });
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

      if (act.action === 'place_bomb') {
        this.handlePlaceBomb(player, stepEvents);
      } else if (act.action === 'use_item') {
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

    // Choose equipped explosive
    const smallCount = player.inventory.items['small_charge'] ?? 0;
    const heavyCount = player.inventory.items['heavy_charge'] ?? 0;
    const mineCount = player.inventory.items['proximity_mine'] ?? 0;

    if (smallCount > 0) {
      const activeCount = this.explosives.filter((e) => e.ownerId === player.id).length;
      if (activeCount >= (EQUIPMENT.small_charge.maxActive ?? 2)) return;

      player.inventory.items['small_charge'] = smallCount - 1;
      const id = `exp_${++this.explosiveIdCounter}`;
      const explodeAt = this.simTime + (EQUIPMENT.small_charge.fuseMs ?? 1800);
      const exp: SimExplosive = {
        id,
        ownerId: player.id,
        definitionId: 'small_charge',
        tileX,
        tileY,
        explodeAt,
        radius: EQUIPMENT.small_charge.blastRadiusTiles ?? 2,
        damage: EQUIPMENT.small_charge.damage ?? 70,
        penetration: EQUIPMENT.small_charge.penetration ?? 0,
      };
      this.explosives.push(exp);

      stepEvents.push({
        kind: 'explosive_placed',
        id,
        ownerId: player.id,
        definitionId: 'small_charge',
        tileX,
        tileY,
        explodeAt,
      });
    } else if (heavyCount > 0) {
      const activeCount = this.explosives.filter((e) => e.ownerId === player.id).length;
      if (activeCount >= (EQUIPMENT.heavy_charge.maxActive ?? 1)) return;

      player.inventory.items['heavy_charge'] = heavyCount - 1;
      const id = `exp_${++this.explosiveIdCounter}`;
      const explodeAt = this.simTime + (EQUIPMENT.heavy_charge.fuseMs ?? 2400);
      const exp: SimExplosive = {
        id,
        ownerId: player.id,
        definitionId: 'heavy_charge',
        tileX,
        tileY,
        explodeAt,
        radius: EQUIPMENT.heavy_charge.blastRadiusTiles ?? 4,
        damage: EQUIPMENT.heavy_charge.damage ?? 100,
        penetration: EQUIPMENT.heavy_charge.penetration ?? 0,
      };
      this.explosives.push(exp);

      stepEvents.push({
        kind: 'explosive_placed',
        id,
        ownerId: player.id,
        definitionId: 'heavy_charge',
        tileX,
        tileY,
        explodeAt,
      });
    } else if (mineCount > 0) {
      const activeCount = this.mines.filter((m) => m.ownerId === player.id).length;
      if (activeCount >= (EQUIPMENT.proximity_mine.maxActive ?? 2)) return;

      player.inventory.items['proximity_mine'] = mineCount - 1;
      const id = `mine_${++this.explosiveIdCounter}`;
      const armedAt = this.simTime + (EQUIPMENT.proximity_mine.cooldownMs ?? 1000);
      const mine: SimMine = {
        id,
        ownerId: player.id,
        definitionId: 'proximity_mine',
        tileX,
        tileY,
        armedAt,
        triggerRadiusUnits: 768, // 0.75 tile
        radius: EQUIPMENT.proximity_mine.blastRadiusTiles ?? 1,
        damage: EQUIPMENT.proximity_mine.damage ?? 100,
      };
      this.mines.push(mine);

      stepEvents.push({
        kind: 'explosive_placed',
        id,
        ownerId: player.id,
        definitionId: 'proximity_mine',
        tileX,
        tileY,
        explodeAt: armedAt,
      });
    } else {
      // Pickaxe Melee Attack when out of explosives or in close melee range
      let target: SimPlayer | null = null;
      let minDistance = 1024; // 1 tile range

      for (const opp of this.players) {
        if (opp.id === player.id || !opp.alive) continue;
        const dist = Math.hypot(opp.x - player.x, opp.y - player.y);
        if (dist <= minDistance) {
          minDistance = dist;
          target = opp;
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
          player.cash += KILL_BOUNTY;
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
  }

  private handleUseItem(player: SimPlayer, stepEvents: GameEvent[]): void {
    const medKits = player.inventory.items['med_kit'] ?? 0;
    if (medKits > 0 && player.hp < MAX_HP) {
      player.inventory.items['med_kit'] = medKits - 1;
      player.hp = Math.min(MAX_HP, player.hp + (EQUIPMENT.med_kit.heal ?? 40));
      stepEvents.push({
        kind: 'damage',
        playerId: player.id,
        sourceId: 'med_kit',
        amount: -40,
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
