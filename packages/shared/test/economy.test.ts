import { describe, expect, it } from 'vitest';
import { calculateMatchStandings, processPurchase } from '../src/game/economy';
import type { SimPlayer } from '../src/game/types';

describe('Economy & Standings', () => {
  function createPlayer(id: string, cash: number): SimPlayer {
    return {
      id,
      name: id,
      x: 0,
      y: 0,
      hp: 100,
      alive: true,
      cash,
      inventory: { selectedSlot: 0, items: {}, upgrades: {} },
      input: { dx: 0, dy: 0, seq: 0, clientTime: 0, lastReceivedAt: 0 },
      invulnerableUntil: 0,
      stats: { kills: 0, treasureValue: 0, roundWins: 0 },
      digTargetTile: null,
    };
  }

  it('allows valid purchase when player has enough cash', () => {
    const player = createPlayer('p1', 500);
    // Small charge costs 120
    const res = processPurchase(player, 'small_charge', 2);
    expect(res.accepted).toBe(true);
    expect(player.cash).toBe(500 - 240); // 260
    expect(player.inventory.items['small_charge']).toBe(2);
  });

  it('rejects purchase when cash is insufficient', () => {
    const player = createPlayer('p1', 100);
    const res = processPurchase(player, 'small_charge', 1);
    expect(res.accepted).toBe(false);
    expect(res.code).toBe('INSUFFICIENT_FUNDS');
    expect(player.cash).toBe(100);
  });

  it('enforces maxOwned cap for upgrades (e.g. Pickaxe II)', () => {
    const player = createPlayer('p1', 2000);
    const res1 = processPurchase(player, 'pickaxe_2', 1);
    expect(res1.accepted).toBe(true);
    expect(player.inventory.upgrades['pickaxe_2']).toBe(1);

    // Try buying second Pickaxe II (maxOwned: 1)
    const res2 = processPurchase(player, 'pickaxe_2', 1);
    expect(res2.accepted).toBe(false);
    expect(res2.code).toBe('MAX_OWNED_REACHED');
  });

  it('calculates tie-broken standings according to spec (cash -> wins -> kills -> treasure)', () => {
    const p1 = createPlayer('p1', 1000);
    p1.stats = { roundWins: 2, kills: 3, treasureValue: 500 };

    const p2 = createPlayer('p2', 1000);
    p2.stats = { roundWins: 2, kills: 5, treasureValue: 200 }; // More kills!

    const p3 = createPlayer('p3', 1200); // Highest cash
    p3.stats = { roundWins: 1, kills: 1, treasureValue: 800 };

    const standings = calculateMatchStandings([p1, p2, p3]);
    expect(standings[0]?.playerId).toBe('p3'); // 1200 cash
    expect(standings[1]?.playerId).toBe('p2'); // 1000 cash, 5 kills
    expect(standings[2]?.playerId).toBe('p1'); // 1000 cash, 3 kills
  });
});
