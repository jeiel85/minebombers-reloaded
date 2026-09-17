import { describe, expect, it } from 'vitest';
import { CLASSIC_MAP_NAMES, CLASSIC_MAPS, getClassicMap, isValidClassicMap } from '../src/game/classicMaps';
import { generateClassicMine } from '../src/game/mapGenerator';
import { MAP_HEIGHT, MAP_WIDTH } from '../src/game/constants';

describe('Official DOS Mine Bombers Classic Maps', () => {
  it('Loads all 46 official classic maps from res/minebomb data', () => {
    expect(CLASSIC_MAP_NAMES.length).toBe(46);
    expect(CLASSIC_MAP_NAMES).toContain('BATTLE');
    expect(CLASSIC_MAP_NAMES).toContain('CASTLE');
    expect(CLASSIC_MAP_NAMES).toContain('CRUMBLE');
    expect(CLASSIC_MAP_NAMES).toContain('EGYPTI');
    expect(CLASSIC_MAP_NAMES).toContain('LABYRINT');
    expect(CLASSIC_MAP_NAMES).toContain('BOULDER');
  });

  it('Validates map dimensions and tile integrity for classic maps', () => {
    for (const name of CLASSIC_MAP_NAMES) {
      const map = getClassicMap(name, 4);
      expect(map).not.toBeNull();
      expect(map!.width).toBe(MAP_WIDTH);
      expect(map!.height).toBe(MAP_HEIGHT);
      expect(map!.tiles.length).toBe(MAP_WIDTH * MAP_HEIGHT);
      expect(map!.spawns.length).toBe(4);

      // Verify spawns are within bounds and on floor
      for (const sp of map!.spawns) {
        expect(sp.x).toBeGreaterThan(0);
        expect(sp.x).toBeLessThan(MAP_WIDTH - 1);
        expect(sp.y).toBeGreaterThan(0);
        expect(sp.y).toBeLessThan(MAP_HEIGHT - 1);
        expect(map!.tiles[sp.y * MAP_WIDTH + sp.x]).toBe('floor');
      }
    }
  });

  it('generateClassicMine loads preset map when specified', () => {
    expect(isValidClassicMap('BATTLE')).toBe(true);
    expect(isValidClassicMap('UNKNOWN_MAP')).toBe(false);

    const battleMap = generateClassicMine(999, 4, 'BATTLE');
    expect(battleMap.width).toBe(64);
    expect(battleMap.height).toBe(45);
    expect(battleMap.spawns.length).toBe(4);

    const castleMap = generateClassicMine(999, 4, 'CASTLE');
    expect(castleMap.width).toBe(64);
    expect(castleMap.height).toBe(45);
  });
});
