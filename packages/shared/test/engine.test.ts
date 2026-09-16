import { describe, expect, it } from 'vitest';
import { WorldSimulation } from '../src/game/engine';
import { generateClassicMine } from '../src/game/mapGenerator';
import { createDefaultInventory } from '../src/game/economy';
import { KILL_BOUNTY, SURVIVOR_BONUS, WORLD_UNITS_PER_TILE } from '../src/game/constants';

describe('WorldSimulation Headless Engine', () => {
  it('runs a full gameplay sequence: movement, bomb placement, elimination, and survivor bonus', () => {
    const seed = 42;
    const map = generateClassicMine(seed, 2);

    const players = [
      { id: 'p1', name: 'Alice', cash: 500, inventory: createDefaultInventory() },
      { id: 'p2', name: 'Bob', cash: 500, inventory: createDefaultInventory() },
    ];

    const sim = new WorldSimulation(seed, map, players, 1, 60_000);
    expect(sim.players.length).toBe(2);
    expect(sim.phase).toBe('playing');

    // Initially invulnerable for 1500 ms (30 steps of 50ms)
    for (let i = 0; i < 30; i++) {
      sim.step(50);
    }
    expect(sim.simTime).toBe(1500);

    // Both players are now vulnerable
    // Teleport p1 and p2 adjacent on open floor for test
    const p1 = sim.players[0]!;
    const p2 = sim.players[1]!;
    p1.x = 5 * WORLD_UNITS_PER_TILE + 512;
    p1.y = 5 * WORLD_UNITS_PER_TILE + 512;
    p2.x = 5 * WORLD_UNITS_PER_TILE + 512;
    p2.y = 6 * WORLD_UNITS_PER_TILE + 512;

    // Clear tiles at (5,5) and (5,6) to be floor
    sim.tiles[5 * 31 + 5] = { kind: 'floor', durability: 0 };
    sim.tiles[6 * 31 + 5] = { kind: 'floor', durability: 0 };

    // p2 starts with 50 HP for quick elimination test
    p2.hp = 50;

    // p1 places a small_charge (fuse 1800ms = 36 steps)
    sim.queueAction('p1', 'place_bomb', 0, 1);
    sim.step(50);

    expect(sim.explosives.length).toBe(1);
    const exp = sim.explosives[0]!;
    expect(exp.definitionId).toBe('small_charge');
    expect(exp.tileX).toBe(5);
    expect(exp.tileY).toBe(5);

    // Step until explosion (about 36 steps)
    while (sim.explosives.length > 0) {
      sim.step(50);
    }

    // Explosion occurred!
    // p2 was at (5,6) within blast radius 2 -> took 70 dmg -> 50 - 70 <= 0 -> eliminated!
    expect(p2.hp).toBe(0);
    expect(p2.alive).toBe(false);

    // p1 killed p2 -> got kill bounty (+400)
    // plus round ended due to last_survivor -> got survivor bonus (+150)!
    expect(p1.alive).toBe(true);
    expect(p1.cash).toBe(500 + KILL_BOUNTY + SURVIVOR_BONUS);
    expect(p1.stats.kills).toBe(1);
    expect(p1.stats.roundWins).toBe(1);

    // Phase transitioned to round_result
    expect(sim.phase).toBe('round_result');
  });

  it('performs pickaxe melee attack when player has 0 bombs', () => {
    const seed = 99;
    const map = generateClassicMine(seed, 2);
    const p1Inv = createDefaultInventory();
    p1Inv.items['small_charge'] = 0; // 0 bombs!

    const players = [
      { id: 'p1', name: 'Alice', cash: 500, inventory: p1Inv },
      { id: 'p2', name: 'Bob', cash: 500, inventory: createDefaultInventory() },
    ];

    const sim = new WorldSimulation(seed, map, players, 1, 60_000);
    // Advance past invulnerability
    for (let i = 0; i < 30; i++) sim.step(50);

    const p1 = sim.players[0]!;
    const p2 = sim.players[1]!;
    p1.x = 5 * WORLD_UNITS_PER_TILE + 512;
    p1.y = 5 * WORLD_UNITS_PER_TILE + 512;
    p2.x = 5 * WORLD_UNITS_PER_TILE + 700; // adjacent close melee range
    p2.y = 5 * WORLD_UNITS_PER_TILE + 512;
    p2.hp = 25;

    // p1 attempts to place_bomb (with 0 bombs, becomes pickaxe melee swing)
    sim.queueAction('p1', 'place_bomb', 0, 1);
    const events = sim.step(50);

    // p2 takes 25 pickaxe damage and gets eliminated
    expect(p2.hp).toBe(0);
    expect(p2.alive).toBe(false);
    expect(events.some((e) => e.kind === 'damage' && e.sourceId === 'pickaxe')).toBe(true);
    expect(events.some((e) => e.kind === 'player_eliminated')).toBe(true);
    expect(p1.cash).toBe(500 + KILL_BOUNTY + SURVIVOR_BONUS);
  });

  it('collects ammo and med kit pickups correctly', () => {
    const seed = 123;
    const map = generateClassicMine(seed, 2);
    const p1Inv = createDefaultInventory();
    p1Inv.items['small_charge'] = 0;

    const players = [
      { id: 'p1', name: 'Alice', cash: 500, inventory: p1Inv },
    ];

    const sim = new WorldSimulation(seed, map, players, 1, 60_000);
    const p1 = sim.players[0]!;
    p1.x = 4 * WORLD_UNITS_PER_TILE + 512;
    p1.y = 4 * WORLD_UNITS_PER_TILE + 512;

    // Add an ammo pickup at (4,4)
    sim.pickups.push({
      id: 'pickup_ammo_1',
      tileX: 4,
      tileY: 4,
      definitionId: 'ammo',
      collected: false,
    });

    const events = sim.step(50);
    expect(p1.inventory.items['small_charge']).toBe(2);
    expect(sim.pickups[0]!.collected).toBe(true);
    expect(events.some((e) => e.kind === 'pickup_collected' && e.definitionId === 'ammo')).toBe(true);
  });
});

