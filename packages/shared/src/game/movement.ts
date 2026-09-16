import {
  BASE_MOVE_SPEED_UNITS_PER_SEC,
  MAP_HEIGHT,
  MAP_WIDTH,
  WORLD_UNITS_PER_TILE,
} from './constants';
import {
  HALF_HITBOX,
  checkCollision,
  getPlayerAABB,
  tileIndexOf,
} from './collision';
import type { SimPlayer, SimTileState } from './types';
import type { TileKind } from '../protocol';

export interface MovementResult {
  x: number;
  y: number;
  contactedSoilTile: { x: number; y: number } | null;
}

export function movePlayerWithCollision(
  currentX: number,
  currentY: number,
  dx: -1 | 0 | 1,
  dy: -1 | 0 | 1,
  stepMs: number,
  tiles: readonly SimTileState[] | readonly TileKind[],
): MovementResult {
  if (dx === 0 && dy === 0) {
    return { x: currentX, y: currentY, contactedSoilTile: null };
  }

  const baseDist = BASE_MOVE_SPEED_UNITS_PER_SEC * (stepMs / 1000);
  const dist = (dx !== 0 && dy !== 0) ? Math.round(baseDist / Math.SQRT2) : Math.round(baseDist);

  let newX = currentX;
  let newY = currentY;
  let contactedSoil: { x: number; y: number } | null = null;

  // 1. Move along X
  if (dx !== 0) {
    const tryX = currentX + dx * dist;
    const boxX = getPlayerAABB(tryX, newY);
    const colX = checkCollision(boxX, tiles);
    if (!colX.collides) {
      newX = tryX;
    } else {
      // Find boundary
      if (dx > 0) {
        const hitTileX = Math.min(...colX.hitTiles.map((t) => t.x));
        newX = hitTileX * WORLD_UNITS_PER_TILE - HALF_HITBOX;
      } else {
        const hitTileX = Math.max(...colX.hitTiles.map((t) => t.x));
        newX = (hitTileX + 1) * WORLD_UNITS_PER_TILE + HALF_HITBOX;
      }
      const soilHit = colX.hitTiles.find((t) => t.kind === 'soil');
      if (soilHit) {
        contactedSoil = { x: soilHit.x, y: soilHit.y };
      }
    }
  }

  // 2. Move along Y
  if (dy !== 0) {
    const tryY = newY + dy * dist;
    const boxY = getPlayerAABB(newX, tryY);
    const colY = checkCollision(boxY, tiles);
    if (!colY.collides) {
      newY = tryY;
    } else {
      if (dy > 0) {
        const hitTileY = Math.min(...colY.hitTiles.map((t) => t.y));
        newY = hitTileY * WORLD_UNITS_PER_TILE - HALF_HITBOX;
      } else {
        const hitTileY = Math.max(...colY.hitTiles.map((t) => t.y));
        newY = (hitTileY + 1) * WORLD_UNITS_PER_TILE + HALF_HITBOX;
      }
      const soilHit = colY.hitTiles.find((t) => t.kind === 'soil');
      if (soilHit) {
        contactedSoil = { x: soilHit.x, y: soilHit.y };
      }
    }
  }

  // Ensure within arena hard bounds
  newX = Math.max(
    WORLD_UNITS_PER_TILE + HALF_HITBOX,
    Math.min((MAP_WIDTH - 1) * WORLD_UNITS_PER_TILE - HALF_HITBOX, newX),
  );
  newY = Math.max(
    WORLD_UNITS_PER_TILE + HALF_HITBOX,
    Math.min((MAP_HEIGHT - 1) * WORLD_UNITS_PER_TILE - HALF_HITBOX, newY),
  );

  return { x: newX, y: newY, contactedSoilTile: contactedSoil };
}
