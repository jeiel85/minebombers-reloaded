import {
  MAP_HEIGHT,
  MAP_WIDTH,
  PLAYER_HITBOX_UNITS,
  WORLD_UNITS_PER_TILE,
} from './constants';
import type { SimTileState } from './types';
import type { TileKind } from '../protocol';

export const HALF_HITBOX = PLAYER_HITBOX_UNITS / 2; // 288

export interface AABB {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

export function getPlayerAABB(x: number, y: number): AABB {
  return {
    minX: x - HALF_HITBOX,
    minY: y - HALF_HITBOX,
    maxX: x + HALF_HITBOX,
    maxY: y + HALF_HITBOX,
  };
}

export function aabbIntersects(a: AABB, b: AABB): boolean {
  return a.minX < b.maxX && a.maxX > b.minX && a.minY < b.maxY && a.maxY > b.minY;
}

export function tileIndexOf(tileX: number, tileY: number): number {
  return tileY * MAP_WIDTH + tileX;
}

export function isTileSolid(kind: TileKind): boolean {
  return kind === 'rock' || kind === 'soil';
}

export function getTileAABB(tileX: number, tileY: number): AABB {
  return {
    minX: tileX * WORLD_UNITS_PER_TILE,
    minY: tileY * WORLD_UNITS_PER_TILE,
    maxX: (tileX + 1) * WORLD_UNITS_PER_TILE,
    maxY: (tileY + 1) * WORLD_UNITS_PER_TILE,
  };
}

/**
 * Checks if an AABB collides with solid tiles or map outer boundaries.
 */
export function checkCollision(
  box: AABB,
  tiles: readonly SimTileState[] | readonly TileKind[],
): { collides: boolean; hitTiles: Array<{ x: number; y: number; kind: TileKind }> } {
  const hitTiles: Array<{ x: number; y: number; kind: TileKind }> = [];

  // Out-of-bounds collision
  if (
    box.minX < 0 ||
    box.maxX > MAP_WIDTH * WORLD_UNITS_PER_TILE ||
    box.minY < 0 ||
    box.maxY > MAP_HEIGHT * WORLD_UNITS_PER_TILE
  ) {
    return { collides: true, hitTiles };
  }

  const startTileX = Math.max(0, Math.floor(box.minX / WORLD_UNITS_PER_TILE));
  const endTileX = Math.min(MAP_WIDTH - 1, Math.floor((box.maxX - 1) / WORLD_UNITS_PER_TILE));
  const startTileY = Math.max(0, Math.floor(box.minY / WORLD_UNITS_PER_TILE));
  const endTileY = Math.min(MAP_HEIGHT - 1, Math.floor((box.maxY - 1) / WORLD_UNITS_PER_TILE));

  for (let ty = startTileY; ty <= endTileY; ty++) {
    for (let tx = startTileX; tx <= endTileX; tx++) {
      const idx = tileIndexOf(tx, ty);
      const rawTile = tiles[idx];
      const kind: TileKind = typeof rawTile === 'object' && rawTile !== null ? (rawTile as SimTileState).kind : (rawTile as TileKind);
      if (isTileSolid(kind)) {
        hitTiles.push({ x: tx, y: ty, kind });
      }
    }
  }

  return {
    collides: hitTiles.length > 0,
    hitTiles,
  };
}
