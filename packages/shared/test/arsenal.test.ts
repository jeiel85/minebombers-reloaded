import { describe, expect, it } from 'vitest';
import { WorldSimulation } from '../src/game/engine';
import { generateClassicMine } from '../src/game/mapGenerator';
import { createDefaultInventory } from '../src/game/economy';
import { MAP_WIDTH, WORLD_UNITS_PER_TILE } from '../src/game/constants';
import { getPickaxeMultiplier } from '../src/game/digging';

describe('Classic Mine Bombers Arsenal & Mechanics', () => {
  it('Power Drill provides 2.5x excavation speed', () => {
    const inv = createDefaultInventory();
    inv.upgrades['power_drill'] = 1;
    const player = {
      id: 'p1',
      name: 'Driller',
      cash: 1000,
      inventory: inv,
      x: 0,
      y: 0,
      hp: 100,
      alive: true,
      input: { dx: 0, dy: 0, seq: 0, clientTime: 0, lastReceivedAt: 0 },
      invulnerableUntil: 0,
      stats: { kills: 0, treasureValue: 0, roundWins: 0 },
      digTargetTile: null,
    };
    expect(getPickaxeMultiplier(player as any)).toBe(2.5);
  });

  it('Rocket projectile fires, travels across floor, and explodes upon hitting rock/wall', () => {
    const seed = 1234;
    const map = generateClassicMine(seed, 2);
    const p1Inv = createDefaultInventory();
    p1Inv.items['rocket'] = 1;

    const players = [
      { id: 'p1', name: 'Rocketier', cash: 500, inventory: p1Inv },
    ];
    const sim = new WorldSimulation(seed, map, players, 1, 60_000);

    const p1 = sim.players[0]!;
    p1.x = 5 * WORLD_UNITS_PER_TILE + 512;
    p1.y = 5 * WORLD_UNITS_PER_TILE + 512;
    p1.input.dx = 1;
    p1.input.dy = 0;
    p1.inventory.selectedSlot = 5;

    // Clear a corridor for flight
    sim.tiles[5 * MAP_WIDTH + 5] = { kind: 'floor', durability: 0 };
    sim.tiles[5 * MAP_WIDTH + 6] = { kind: 'floor', durability: 0 };
    sim.tiles[5 * MAP_WIDTH + 7] = { kind: 'floor', durability: 0 };
    sim.tiles[5 * MAP_WIDTH + 8] = { kind: 'rock', durability: 0 };

    sim.queueAction('p1', 'place_bomb', 5, 1);
    sim.step(50);

    expect(sim.projectiles.length).toBe(1);
    const rocket = sim.projectiles[0]!;
    expect(rocket.definitionId).toBe('rocket');
    expect(rocket.vx).toBe(6000);

    let steps = 0;
    while (sim.projectiles.length > 0 && steps < 30) {
      sim.step(50);
      steps++;
    }
    expect(sim.projectiles.length).toBe(0);
  });

  it('Remote Bomb is placed with infinite timer and detonates when secondary action is queued', () => {
    const seed = 555;
    const map = generateClassicMine(seed, 2);
    const p1Inv = createDefaultInventory();
    p1Inv.items['remote_bomb'] = 1;

    const players = [
      { id: 'p1', name: 'Trapper', cash: 500, inventory: p1Inv },
    ];
    const sim = new WorldSimulation(seed, map, players, 1, 60_000);

    const p1 = sim.players[0]!;
    p1.inventory.selectedSlot = 3;
    sim.queueAction('p1', 'place_bomb', 3, 1);
    sim.step(50);

    expect(sim.explosives.length).toBe(1);
    const bomb = sim.explosives[0]!;
    expect(bomb.isRemote).toBe(true);
    expect(bomb.explodeAt).toBe(Infinity);

    sim.queueAction('p1', 'use_secondary', 0, 2);
    sim.step(50);

    expect(sim.explosives.length).toBe(0);
  });

  it('Teleporter warps player to a safe cavern floor tile', () => {
    const seed = 777;
    const map = generateClassicMine(seed, 2);
    const p1Inv = createDefaultInventory();
    p1Inv.items['teleport'] = 1;

    const players = [
      { id: 'p1', name: 'Jumper', cash: 500, inventory: p1Inv },
    ];
    const sim = new WorldSimulation(seed, map, players, 1, 60_000);

    const p1 = sim.players[0]!;
    const originalX = p1.x;
    const originalY = p1.y;

    sim.queueAction('p1', 'use_secondary', 0, 1);
    const events = sim.step(50);

    expect(events.some((e) => e.kind === 'teleported')).toBe(true);
    expect(p1.inventory.items['teleport']).toBe(0);
    const moved = p1.x !== originalX || p1.y !== originalY;
    expect(moved).toBe(true);
  });

  it('Nuclear Warhead (Nuke) creates a massive 8-tile explosion destroying wide soil and rocks', () => {
    const seed = 888;
    const map = generateClassicMine(seed, 2);
    const p1Inv = createDefaultInventory();
    p1Inv.items['nuke'] = 1;

    const players = [
      { id: 'p1', name: 'Bomber', cash: 2000, inventory: p1Inv },
    ];
    const sim = new WorldSimulation(seed, map, players, 1, 60_000);

    const p1 = sim.players[0]!;
    p1.inventory.selectedSlot = 7;
    sim.queueAction('p1', 'place_bomb', 7, 1);
    sim.step(50);

    expect(sim.explosives.length).toBe(1);
    const nuke = sim.explosives[0]!;
    expect(nuke.radius).toBe(8);
    expect(nuke.damage).toBe(200);

    while (sim.explosives.length > 0) {
      sim.step(50);
    }

    const expEvent = sim.recentEvents.find((e) => e.kind === 'explosion' && e.cells.length > 25);
    expect(expEvent).toBeDefined();
  });

  it('Cavern monsters roam and deal contact damage to miners', () => {
    const seed = 999;
    const map = generateClassicMine(seed, 2);
    const players = [
      { id: 'p1', name: 'Explorer', cash: 500, inventory: createDefaultInventory() },
    ];
    const sim = new WorldSimulation(seed, map, players, 1, 60_000);
    expect(sim.monsters.length).toBeGreaterThan(0);

    const monster = sim.monsters[0]!;
    const p1 = sim.players[0]!;
    p1.invulnerableUntil = 0; // Past invulnerability

    // Position monster right on player
    monster.x = p1.x;
    monster.y = p1.y;

    const events = sim.step(50);
    expect(p1.hp).toBeLessThan(100);
    expect(events.some((e) => e.kind === 'damage' && (e.sourceId === 'slime' || e.sourceId === 'bat'))).toBe(true);
  });

  it('Triggers Sudden Death Mine Collapse when round timer reaches 45 seconds remaining', () => {
    const seed = 111;
    const map = generateClassicMine(seed, 2);
    const players = [
      { id: 'p1', name: 'Survivor', cash: 500, inventory: createDefaultInventory() },
    ];
    // Start with 46 seconds duration
    const sim = new WorldSimulation(seed, map, players, 1, 46_000);
    expect(sim.isCollapsing).toBe(false);

    // Step 1500ms (so remaining is < 45s)
    for (let i = 0; i < 30; i++) {
      sim.step(50);
    }

    expect(sim.isCollapsing).toBe(true);
    expect(sim.recentEvents.some((e) => e.kind === 'mine_collapse')).toBe(true);
  });
});

