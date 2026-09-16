import { describe, expect, it } from 'vitest';
import { computeExplosion } from '../src/game/explosions';
import { BASE_SOIL_DURABILITY, KILL_BOUNTY, MAP_WIDTH, WORLD_UNITS_PER_TILE } from '../src/game/constants';
import type { SimExplosive, SimPlayer, SimTileState } from '../src/game/types';

describe('Explosions & Raycast damage', () => {
  const width = MAP_WIDTH;
  const height = 23;

  function createMap(): SimTileState[] {
    return Array.from({ length: width * height }, () => ({
      kind: 'soil',
      durability: BASE_SOIL_DURABILITY,
    }));
  }

  function createPlayer(id: string, xTiles: number, yTiles: number, hp = 100): SimPlayer {
    return {
      id,
      name: id,
      x: (xTiles + 0.5) * WORLD_UNITS_PER_TILE,
      y: (yTiles + 0.5) * WORLD_UNITS_PER_TILE,
      hp,
      alive: true,
      cash: 500,
      inventory: { selectedSlot: 0, items: {}, upgrades: {} },
      input: { dx: 0, dy: 0, seq: 0, clientTime: 0, lastReceivedAt: 0 },
      invulnerableUntil: 0,
      stats: { kills: 0, treasureValue: 0, roundWins: 0 },
      digTargetTile: null,
    };
  }

  it('destroys soil and stops raycast in cardinal directions', () => {
    const tiles = createMap();
    // Explode at (5, 5) with radius 2, Small Charge
    const res = computeExplosion(
      'exp_1',
      'p1',
      5,
      5,
      2,
      70,
      0, // penetration = 0
      1000,
      tiles,
      [],
      [],
      [],
      [],
    );

    // Center (5,5) destroyed
    expect(tiles[5 * width + 5]?.kind).toBe('floor');
    // Distance 1 in each cardinal direction destroyed
    expect(tiles[4 * width + 5]?.kind).toBe('floor'); // Up
    expect(tiles[6 * width + 5]?.kind).toBe('floor'); // Down
    expect(tiles[5 * width + 4]?.kind).toBe('floor'); // Left
    expect(tiles[5 * width + 6]?.kind).toBe('floor'); // Right

    // Distance 2 was blocked by the soil at distance 1!
    expect(tiles[3 * width + 5]?.kind).toBe('soil');
    expect(tiles[7 * width + 5]?.kind).toBe('soil');
  });

  it('damages players in affected blast cells and eliminates at <= 0 HP', () => {
    const tiles = createMap();
    const p1 = createPlayer('p1', 5, 5); // at center
    const p2 = createPlayer('p2', 5, 6, 50); // 1 tile below, HP 50

    const res = computeExplosion(
      'exp_1',
      'p1',
      5,
      5,
      2,
      70,
      0,
      1000,
      tiles,
      [p1, p2],
      [],
      [],
      [],
    );

    // p1 took 70 dmg: 100 -> 30
    expect(p1.hp).toBe(30);
    expect(p1.alive).toBe(true);

    // p2 took 70 dmg (clamped to 50): 50 -> 0 -> eliminated!
    expect(p2.hp).toBe(0);
    expect(p2.alive).toBe(false);

    // p1 (killer) receives kill bounty
    expect(p1.cash).toBe(500 + KILL_BOUNTY);
    expect(p1.stats.kills).toBe(1);
  });

  it('triggers chain detonation for nearby explosives', () => {
    const tiles = createMap();
    // Clear path so ray reaches
    tiles[5 * width + 5] = { kind: 'floor', durability: 0 };
    tiles[5 * width + 6] = { kind: 'floor', durability: 0 };

    const nearbyExp: SimExplosive = {
      id: 'exp_2',
      ownerId: 'p2',
      definitionId: 'small_charge',
      tileX: 6,
      tileY: 5,
      explodeAt: 5000,
      radius: 2,
      damage: 70,
      penetration: 0,
    };

    const res = computeExplosion(
      'exp_1',
      'p1',
      5,
      5,
      2,
      70,
      0,
      1000,
      tiles,
      [],
      [nearbyExp],
      [],
      [],
    );

    // Chain scheduled at simTime + 50 (1050)
    expect(nearbyExp.explodeAt).toBe(1050);
    expect(res.chainExplosives).toContain(nearbyExp);
  });
});
