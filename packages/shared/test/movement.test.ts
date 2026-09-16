import { describe, expect, it } from 'vitest';
import { movePlayerWithCollision } from '../src/game/movement';
import { WORLD_UNITS_PER_TILE } from '../src/game/constants';
import { HALF_HITBOX } from '../src/game/collision';
import type { TileKind } from '../src/protocol';

describe('Player movement & AABB collision', () => {
  const width = 31;
  const height = 23;
  // Floor everywhere except borders
  const openTiles: TileKind[] = Array.from({ length: width * height }, (_, idx) => {
    const x = idx % width;
    const y = Math.floor(idx / width);
    if (x === 0 || y === 0 || x === width - 1 || y === height - 1) return 'rock';
    return 'floor';
  });

  it('moves freely on open floor', () => {
    const startX = 2 * WORLD_UNITS_PER_TILE + 512;
    const startY = 2 * WORLD_UNITS_PER_TILE + 512;

    const res = movePlayerWithCollision(startX, startY, 1, 0, 50, openTiles);
    // 3000 * 0.05 = 150
    expect(res.x).toBe(startX + 150);
    expect(res.y).toBe(startY);
    expect(res.contactedSoilTile).toBeNull();
  });

  it('normalizes diagonal speed so it is not faster than cardinal', () => {
    const startX = 5 * WORLD_UNITS_PER_TILE;
    const startY = 5 * WORLD_UNITS_PER_TILE;

    const resDiag = movePlayerWithCollision(startX, startY, 1, 1, 50, openTiles);
    const diagDistX = resDiag.x - startX;
    const diagDistY = resDiag.y - startY;

    // 150 / sqrt(2) ≈ 106
    expect(diagDistX).toBe(106);
    expect(diagDistY).toBe(106);
    const totalDist = Math.sqrt(diagDistX * diagDistX + diagDistY * diagDistY);
    expect(Math.round(totalDist)).toBeCloseTo(150, 0);
  });

  it('stops at solid rock border and clamps position', () => {
    // Player near left border (rock is at x=0)
    // Left boundary of tile 1 is 1024. Player center cannot go below 1024 + HALF_HITBOX (1312)
    const startX = 1024 + HALF_HITBOX + 50;
    const startY = 2 * WORLD_UNITS_PER_TILE;

    const res = movePlayerWithCollision(startX, startY, -1, 0, 50, openTiles);
    expect(res.x).toBe(1024 + HALF_HITBOX);
  });

  it('detects contact with soil tile when pushing into it', () => {
    const tilesWithSoil = [...openTiles];
    // Place a soil tile at (3, 2)
    tilesWithSoil[2 * width + 3] = 'soil';

    // Place player at tile (2, 2), center x = 2 * 1024 + 512 = 2560
    // Moving right pushes into (3, 2) soil tile
    const startX = 3 * WORLD_UNITS_PER_TILE - HALF_HITBOX - 10;
    const startY = 2 * WORLD_UNITS_PER_TILE + 512;

    const res = movePlayerWithCollision(startX, startY, 1, 0, 50, tilesWithSoil);
    expect(res.contactedSoilTile).toEqual({ x: 3, y: 2 });
    expect(res.x).toBe(3 * WORLD_UNITS_PER_TILE - HALF_HITBOX);
  });
});
