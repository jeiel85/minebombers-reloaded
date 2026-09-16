import { describe, expect, it } from 'vitest';
import { stepDigging } from '../src/game/digging';
import { BASE_SOIL_DURABILITY } from '../src/game/constants';
import type { SimPlayer, SimTileState, SimTreasureEntity } from '../src/game/types';

describe('Digging progression and soil destruction', () => {
  const width = 31;
  const height = 23;

  function createTestPlayer(pickaxe2 = false): SimPlayer {
    return {
      id: 'p1',
      name: 'Miner',
      x: 2000,
      y: 2000,
      hp: 100,
      alive: true,
      cash: 500,
      inventory: {
        selectedSlot: 0,
        items: {},
        upgrades: pickaxe2 ? { pickaxe_2: 1 } : { pickaxe_1: 1 },
      },
      input: { dx: 1, dy: 0, seq: 1, clientTime: 0, lastReceivedAt: 0 },
      invulnerableUntil: 0,
      stats: { kills: 0, treasureValue: 0, roundWins: 0 },
      digTargetTile: null,
    };
  }

  it('accumulates dig work and turns soil to floor after 1000 work units', () => {
    const tiles: SimTileState[] = Array.from({ length: width * height }, () => ({
      kind: 'soil',
      durability: BASE_SOIL_DURABILITY,
    }));
    const targetIdx = 2 * width + 3;
    const treasures: SimTreasureEntity[] = [
      {
        id: 'tr_1',
        tileIndex: targetIdx,
        tileX: 3,
        tileY: 2,
        rarity: 'basic',
        value: 100,
        revealed: false,
        collected: false,
      },
    ];

    const player = createTestPlayer(false); // Base rate = 1000/s -> 50 work / 50ms step

    // Run 19 steps (950 work units, should not be opened yet)
    for (let i = 0; i < 19; i++) {
      const res = stepDigging(player, { x: 3, y: 2 }, 50, tiles, treasures);
      expect(res).not.toBeNull();
      expect(res!.tileOpened).toBe(false);
      expect(tiles[targetIdx]?.kind).toBe('soil');
    }

    // Step 20 reaches 1000 work units -> opens!
    const finalRes = stepDigging(player, { x: 3, y: 2 }, 50, tiles, treasures);
    expect(finalRes).not.toBeNull();
    expect(finalRes!.tileOpened).toBe(true);
    expect(tiles[targetIdx]?.kind).toBe('floor');
    expect(finalRes!.revealedTreasure).not.toBeNull();
    expect(finalRes!.revealedTreasure?.id).toBe('tr_1');
    expect(treasures[0]?.revealed).toBe(true);
  });

  it('digs faster with Pickaxe II (1.45x)', () => {
    const tiles: SimTileState[] = Array.from({ length: width * height }, () => ({
      kind: 'soil',
      durability: BASE_SOIL_DURABILITY,
    }));
    const targetIdx = 2 * width + 3;
    const player = createTestPlayer(true); // Pickaxe II (1.45x)

    // With 1.45x, 50 * 1.45 = 72.5 -> ~73 work per step
    // 1000 / 72.5 ≈ 14 steps
    let steps = 0;
    while (tiles[targetIdx]?.kind === 'soil' && steps < 30) {
      stepDigging(player, { x: 3, y: 2 }, 50, tiles, []);
      steps++;
    }

    expect(steps).toBeLessThanOrEqual(14);
    expect(tiles[targetIdx]?.kind).toBe('floor');
  });
});
