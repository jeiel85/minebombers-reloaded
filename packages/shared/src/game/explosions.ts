import {
  KILL_BOUNTY,
  MAP_HEIGHT,
  MAP_WIDTH,
  WORLD_UNITS_PER_TILE,
} from './constants';
import { aabbIntersects, getPlayerAABB, getTileAABB, tileIndexOf } from './collision';
import type {
  SimExplosive,
  SimMine,
  SimPlayer,
  SimTileState,
  SimTreasureEntity,
} from './types';
import type { GameEvent } from '../protocol';

export interface ExplosionResult {
  affectedCells: Array<{ x: number; y: number }>;
  openedTiles: number[];
  revealedTreasures: SimTreasureEntity[];
  damagedPlayers: Array<{
    player: SimPlayer;
    amount: number;
    eliminated: boolean;
    killerId: string | null;
  }>;
  chainExplosives: SimExplosive[];
  chainMines: SimMine[];
  events: GameEvent[];
}

export function computeExplosion(
  sourceId: string,
  ownerId: string,
  centerTileX: number,
  centerTileY: number,
  radius: number,
  damage: number,
  penetration: number,
  simTime: number,
  tiles: SimTileState[],
  players: SimPlayer[],
  explosives: SimExplosive[],
  mines: SimMine[],
  treasures: SimTreasureEntity[],
): ExplosionResult {
  const affectedCells: Array<{ x: number; y: number }> = [];
  const openedTiles: number[] = [];
  const revealedTreasures: SimTreasureEntity[] = [];
  const events: GameEvent[] = [];

  // 1. Center tile
  if (centerTileX >= 0 && centerTileX < MAP_WIDTH && centerTileY >= 0 && centerTileY < MAP_HEIGHT) {
    affectedCells.push({ x: centerTileX, y: centerTileY });
    const centerIdx = tileIndexOf(centerTileX, centerTileY);
    const centerTile = tiles[centerIdx];
    if (centerTile && centerTile.kind === 'soil') {
      centerTile.kind = 'floor';
      centerTile.durability = 0;
      openedTiles.push(centerIdx);
      events.push({ kind: 'tile_changed', index: centerIdx, tile: 'floor' });
      const hidden = treasures.find((t) => t.tileIndex === centerIdx && !t.revealed && !t.collected);
      if (hidden) {
        hidden.revealed = true;
        revealedTreasures.push(hidden);
      }
    }
  }

  // 2. Cardinal rays
  const directions = [
    { dx: 0, dy: -1 },
    { dx: 0, dy: 1 },
    { dx: -1, dy: 0 },
    { dx: 1, dy: 0 },
  ];

  for (const dir of directions) {
    let extraPenetration = penetration;
    for (let dist = 1; dist <= radius; dist++) {
      const cx = centerTileX + dir.dx * dist;
      const cy = centerTileY + dir.dy * dist;
      if (cx < 0 || cx >= MAP_WIDTH || cy < 0 || cy >= MAP_HEIGHT) break;

      const idx = tileIndexOf(cx, cy);
      const tile = tiles[idx];
      if (!tile) break;

      // Rock blocks ray unconditionally
      if (tile.kind === 'rock') {
        break;
      }

      // Soil is affected and destroyed
      if (tile.kind === 'soil') {
        affectedCells.push({ x: cx, y: cy });
        tile.kind = 'floor';
        tile.durability = 0;
        openedTiles.push(idx);
        events.push({ kind: 'tile_changed', index: idx, tile: 'floor' });

        const hidden = treasures.find((t) => t.tileIndex === idx && !t.revealed && !t.collected);
        if (hidden) {
          hidden.revealed = true;
          revealedTreasures.push(hidden);
        }

        if (extraPenetration > 0) {
          extraPenetration--;
        } else {
          break; // Stop ray
        }
      } else {
        // Floor or other passable
        affectedCells.push({ x: cx, y: cy });
      }
    }
  }

  events.push({
    kind: 'explosion',
    id: sourceId,
    sourceId,
    cells: affectedCells,
    at: simTime,
  });

  // 3. Chain detonations
  const chainExplosives: SimExplosive[] = [];
  const chainMines: SimMine[] = [];

  for (const cell of affectedCells) {
    for (const exp of explosives) {
      if (exp.id === sourceId) continue;
      if (exp.tileX === cell.x && exp.tileY === cell.y) {
        if (exp.explodeAt > simTime + 50) {
          exp.explodeAt = simTime + 50;
          chainExplosives.push(exp);
        }
      }
    }
    for (const mine of mines) {
      if (mine.id === sourceId) continue;
      if (mine.tileX === cell.x && mine.tileY === cell.y) {
        if (!mine.detonated) {
          mine.detonated = true;
          chainMines.push(mine);
        }
      }
    }
  }

  // 4. Damage players in affected cells
  const damagedPlayers: Array<{
    player: SimPlayer;
    amount: number;
    eliminated: boolean;
    killerId: string | null;
  }> = [];

  for (const player of players) {
    if (!player.alive) continue;
    if (simTime < player.invulnerableUntil) continue; // Spawn invulnerability

    const pBox = getPlayerAABB(player.x, player.y);
    const inAffected = affectedCells.some((cell) => {
      const cellBox = getTileAABB(cell.x, cell.y);
      return aabbIntersects(pBox, cellBox);
    });

    if (inAffected) {
      const actualDmg = Math.min(player.hp, damage);
      player.hp = Math.max(0, player.hp - damage);
      const eliminated = player.hp <= 0;
      if (eliminated) {
        player.alive = false;
      }

      const killerId = eliminated ? (ownerId || null) : null;
      damagedPlayers.push({
        player,
        amount: actualDmg,
        eliminated,
        killerId,
      });

      events.push({
        kind: 'damage',
        playerId: player.id,
        sourceId,
        amount: actualDmg,
        hpAfter: player.hp,
      });

      if (eliminated) {
        events.push({
          kind: 'player_eliminated',
          playerId: player.id,
          killerPlayerId: killerId,
        });

        // Kill bounty if eliminated by another player
        if (killerId && killerId !== player.id) {
          const killer = players.find((p) => p.id === killerId);
          if (killer) {
            killer.cash += KILL_BOUNTY;
            killer.stats.kills += 1;
            events.push({
              kind: 'cash_changed',
              playerId: killer.id,
              cash: killer.cash,
              reason: 'bounty',
            });
          }
        }
      }
    }
  }

  return {
    affectedCells,
    openedTiles,
    revealedTreasures,
    damagedPlayers,
    chainExplosives,
    chainMines,
    events,
  };
}
