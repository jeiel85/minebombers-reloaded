import { describe, expect, it } from 'vitest';
import { generateClassicMine, MAP_GENERATOR_ID } from '../src/game/mapGenerator';
import { MAP_HEIGHT, MAP_WIDTH } from '../src/game/constants';

describe('classic-mine-v1 map generator', () => {
  it('generates deterministic maps for fixed seeds', () => {
    const mapA = generateClassicMine(12345, 4);
    const mapB = generateClassicMine(12345, 4);

    expect(mapA.generator).toBe(MAP_GENERATOR_ID);
    expect(mapA.seed).toBe(12345);
    expect(mapA.width).toBe(MAP_WIDTH);
    expect(mapA.height).toBe(MAP_HEIGHT);
    expect(mapA.tiles).toEqual(mapB.tiles);
    expect(mapA.spawns).toEqual(mapB.spawns);
    expect(mapA.treasures).toEqual(mapB.treasures);
  });

  it('generates outer rock border', () => {
    const map = generateClassicMine(1, 2);
    for (let x = 0; x < MAP_WIDTH; x++) {
      expect(map.tiles[x]).toBe('rock'); // y=0
      expect(map.tiles[(MAP_HEIGHT - 1) * MAP_WIDTH + x]).toBe('rock'); // y=MAP_HEIGHT-1
    }
    for (let y = 0; y < MAP_HEIGHT; y++) {
      expect(map.tiles[y * MAP_WIDTH]).toBe('rock'); // x=0
      expect(map.tiles[y * MAP_WIDTH + (MAP_WIDTH - 1)]).toBe('rock'); // x=MAP_WIDTH-1
    }
  });

  it('guarantees spawn safe zones are floor', () => {
    const map = generateClassicMine(42, 4);
    for (const spawn of map.spawns) {
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const x = spawn.x + dx;
          const y = spawn.y + dy;
          if (x > 0 && y > 0 && x < MAP_WIDTH - 1 && y < MAP_HEIGHT - 1) {
            expect(map.tiles[y * MAP_WIDTH + x]).toBe('floor');
          }
        }
      }
    }
  });

  it('guarantees interior rocks are not orthogonally adjacent to other interior rocks', () => {
    const map = generateClassicMine(9999, 8);
    const orthogonal = [[1, 0], [-1, 0], [0, 1], [0, -1]] as const;

    for (let y = 1; y < MAP_HEIGHT - 1; y++) {
      for (let x = 1; x < MAP_WIDTH - 1; x++) {
        if (map.tiles[y * MAP_WIDTH + x] === 'rock') {
          for (const [dx, dy] of orthogonal) {
            const nx = x + dx;
            const ny = y + dy;
            if (nx > 0 && ny > 0 && nx < MAP_WIDTH - 1 && ny < MAP_HEIGHT - 1) {
              expect(map.tiles[ny * MAP_WIDTH + nx]).not.toBe('rock');
            }
          }
        }
      }
    }
  });

  it('generates correct treasure counts based on player count', () => {
    const map4 = generateClassicMine(777, 4);
    // basic: 18 + 2*4 = 26; rare: max(2, 4) = 4; total = 30
    const basicCount = map4.treasures.filter((t) => t.rarity === 'basic').length;
    const rareCount = map4.treasures.filter((t) => t.rarity === 'rare').length;
    expect(basicCount).toBe(26);
    expect(rareCount).toBe(4);
    expect(map4.treasures.length).toBe(30);

    // Ensure all treasures are placed in soil
    for (const t of map4.treasures) {
      expect(map4.tiles[t.index]).toBe('soil');
    }
  });
});
