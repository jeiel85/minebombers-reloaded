import type { TileKind } from '../protocol';
import {
  BASIC_TREASURE_VALUE,
  MAP_HEIGHT,
  MAP_WIDTH,
  RARE_TREASURE_VALUE,
} from './constants';
import { mulberry32, shuffleInPlace } from './prng';

export const MAP_GENERATOR_ID = 'classic-mine-v1';

export interface GridPoint { x: number; y: number }

export interface GeneratedTreasure {
  id: string;
  index: number;
  rarity: 'basic' | 'rare';
  value: number;
}

export interface GeneratedMap {
  generator: typeof MAP_GENERATOR_ID;
  seed: number;
  width: number;
  height: number;
  tiles: TileKind[];
  spawns: GridPoint[];
  treasures: GeneratedTreasure[];
}

export function getSpawnCandidates(): GridPoint[] {
  const w = MAP_WIDTH;
  const h = MAP_HEIGHT;
  return [
    { x: 4, y: 4 },
    { x: w - 5, y: h - 5 },
    { x: w - 5, y: 4 },
    { x: 4, y: h - 5 },
    { x: Math.floor(w / 2), y: 4 },
    { x: Math.floor(w / 2), y: h - 5 },
    { x: 4, y: Math.floor(h / 2) },
    { x: w - 5, y: Math.floor(h / 2) },
  ];
}

function indexOf(x: number, y: number): number {
  return y * MAP_WIDTH + x;
}

function inBounds(x: number, y: number): boolean {
  return x >= 0 && y >= 0 && x < MAP_WIDTH && y < MAP_HEIGHT;
}

function manhattan(a: GridPoint, b: GridPoint): number {
  return Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
}

export function generateClassicMine(seed: number, playerCount: number): GeneratedMap {
  if (!Number.isInteger(playerCount) || playerCount < 2 || playerCount > 8) {
    throw new Error('playerCount must be an integer from 2 to 8');
  }

  const random = mulberry32(seed >>> 0);
  const tiles: TileKind[] = Array.from({ length: MAP_WIDTH * MAP_HEIGHT }, () => 'soil');
  const spawnPoints = getSpawnCandidates();
  const spawns = spawnPoints.slice(0, playerCount).map((p) => ({ ...p }));

  // Outer hard-rock border.
  for (let y = 0; y < MAP_HEIGHT; y++) {
    for (let x = 0; x < MAP_WIDTH; x++) {
      if (x === 0 || y === 0 || x === MAP_WIDTH - 1 || y === MAP_HEIGHT - 1) {
        tiles[indexOf(x, y)] = 'rock';
      }
    }
  }

  // Spawn 3x3 safe zones plus one cell toward center.
  const center = { x: Math.floor(MAP_WIDTH / 2), y: Math.floor(MAP_HEIGHT / 2) };
  for (const spawn of spawns) {
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const x = spawn.x + dx;
        const y = spawn.y + dy;
        if (x > 0 && y > 0 && x < MAP_WIDTH - 1 && y < MAP_HEIGHT - 1) {
          tiles[indexOf(x, y)] = 'floor';
        }
      }
    }
    const stepX = Math.sign(center.x - spawn.x);
    const stepY = Math.sign(center.y - spawn.y);
    const x = spawn.x + (Math.abs(center.x - spawn.x) >= Math.abs(center.y - spawn.y) ? stepX * 2 : 0);
    const y = spawn.y + (Math.abs(center.y - spawn.y) > Math.abs(center.x - spawn.x) ? stepY * 2 : 0);
    if (inBounds(x, y) && x > 0 && y > 0 && x < MAP_WIDTH - 1 && y < MAP_HEIGHT - 1) {
      tiles[indexOf(x, y)] = 'floor';
    }
  }

  // Central 5x5 contest cavern area.
  for (let dy = -2; dy <= 2; dy++) {
    for (let dx = -2; dx <= 2; dx++) {
      tiles[indexOf(center.x + dx, center.y + dy)] = 'floor';
    }
  }

  // Interior isolated rock pillars.
  const candidates: GridPoint[] = [];
  for (let y = 1; y < MAP_HEIGHT - 1; y++) {
    for (let x = 1; x < MAP_WIDTH - 1; x++) {
      const p = { x, y };
      if (tiles[indexOf(x, y)] !== 'soil') continue;
      if (spawns.some((s) => manhattan(s, p) < 5)) continue;
      if (Math.abs(x - center.x) <= 3 && Math.abs(y - center.y) <= 3) continue;
      candidates.push(p);
    }
  }
  shuffleInPlace(candidates, random);

  const rockTarget = Math.floor((MAP_WIDTH - 2) * (MAP_HEIGHT - 2) * 0.08);
  let placedRocks = 0;
  const orthogonal = [[1,0],[-1,0],[0,1],[0,-1]] as const;
  for (const p of candidates) {
    if (placedRocks >= rockTarget) break;
    // Border can be adjacent to interior cells; only reject generated/interior neighbor rocks.
    const adjacentInteriorRock = orthogonal.some(([dx,dy]) => {
      const nx=p.x+dx, ny=p.y+dy;
      return nx > 0 && ny > 0 && nx < MAP_WIDTH-1 && ny < MAP_HEIGHT-1 && tiles[indexOf(nx,ny)] === 'rock';
    });
    if (adjacentInteriorRock) continue;
    tiles[indexOf(p.x, p.y)] = 'rock';
    placedRocks++;
  }

  // Treasure is attached to remaining soil and revealed when that soil opens.
  const treasureCandidates: number[] = [];
  for (let y = 1; y < MAP_HEIGHT - 1; y++) {
    for (let x = 1; x < MAP_WIDTH - 1; x++) {
      if (tiles[indexOf(x, y)] !== 'soil') continue;
      if (spawns.some((s) => manhattan(s, { x, y }) < 4)) continue;
      treasureCandidates.push(indexOf(x, y));
    }
  }
  shuffleInPlace(treasureCandidates, random);

  const rareCount = Math.max(8, playerCount * 2);
  const basicCount = 40 + 6 * playerCount;
  const treasures: GeneratedTreasure[] = [];
  let cursor = 0;
  for (let i = 0; i < rareCount && cursor < treasureCandidates.length; i++, cursor++) {
    const index = treasureCandidates[cursor]!;
    treasures.push({ id: `treasure_r_${index}`, index, rarity: 'rare', value: RARE_TREASURE_VALUE });
  }
  for (let i = 0; i < basicCount && cursor < treasureCandidates.length; i++, cursor++) {
    const index = treasureCandidates[cursor]!;
    treasures.push({ id: `treasure_b_${index}`, index, rarity: 'basic', value: BASIC_TREASURE_VALUE });
  }

  return {
    generator: MAP_GENERATOR_ID,
    seed: seed >>> 0,
    width: MAP_WIDTH,
    height: MAP_HEIGHT,
    tiles,
    spawns,
    treasures,
  };
}
