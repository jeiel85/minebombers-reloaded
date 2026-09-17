import {
  BASIC_TREASURE_VALUE,
  DEFAULT_MATCH_ROUNDS,
  KILL_BOUNTY,
  RARE_TREASURE_VALUE,
  STARTING_CASH,
  SURVIVOR_BONUS,
} from './constants';
import { EQUIPMENT, type EquipmentDef, type EquipmentId } from './equipment';
import type {
  SimMatchStandingsEntry,
  SimPlayer,
  SimRoundStandingsEntry,
} from './types';
import type { InventoryState } from '../protocol';

export function createDefaultInventory(): InventoryState {
  return {
    selectedSlot: 0,
    items: {
      small_charge: 2, // starter gift/stock for vertical slice
    },
    upgrades: {
      pickaxe_1: 1,
    },
  };
}

export interface PurchaseResult {
  accepted: boolean;
  code?: string;
  cash: number;
  inventory: InventoryState;
}

export function processPurchase(
  player: SimPlayer,
  equipmentId: string,
  quantity: number,
): PurchaseResult {
  if (quantity <= 0 || !Number.isInteger(quantity)) {
    return { accepted: false, code: 'INVALID_QUANTITY', cash: player.cash, inventory: player.inventory };
  }

  const def: EquipmentDef | undefined = EQUIPMENT[equipmentId as EquipmentId];
  if (!def) {
    return { accepted: false, code: 'UNKNOWN_EQUIPMENT', cash: player.cash, inventory: player.inventory };
  }

  const totalPrice = def.cost * quantity;
  if (player.cash < totalPrice) {
    return { accepted: false, code: 'INSUFFICIENT_FUNDS', cash: player.cash, inventory: player.inventory };
  }

  // Max owned check (e.g. upgrades)
  const currentCount = (player.inventory.items[equipmentId] ?? 0) + (player.inventory.upgrades[equipmentId] ?? 0);
  if (def.maxOwned !== undefined && currentCount + quantity > def.maxOwned) {
    return { accepted: false, code: 'MAX_OWNED_REACHED', cash: player.cash, inventory: player.inventory };
  }

  // Deduct cash
  player.cash -= totalPrice;

  // Add to inventory
  if (def.kind === 'tool' || def.kind === 'passive') {
    player.inventory.upgrades[equipmentId] = (player.inventory.upgrades[equipmentId] ?? 0) + quantity;
  } else {
    player.inventory.items[equipmentId] = (player.inventory.items[equipmentId] ?? 0) + quantity;
  }

  return {
    accepted: true,
    cash: player.cash,
    inventory: player.inventory,
  };
}

/**
 * Calculates tie-broken standings according to docs/14:
 * 1. Cash (primary)
 * 2. Most round wins
 * 3. Most kills
 * 4. Most treasure value collected
 */
export function calculateMatchStandings(players: SimPlayer[]): SimMatchStandingsEntry[] {
  return [...players]
    .map((p) => ({
      playerId: p.id,
      cash: p.cash,
      roundWins: p.stats.roundWins,
      kills: p.stats.kills,
      treasureValue: p.stats.treasureValue,
    }))
    .sort((a, b) => {
      if (b.cash !== a.cash) return b.cash - a.cash;
      if (b.roundWins !== a.roundWins) return b.roundWins - a.roundWins;
      if (b.kills !== a.kills) return b.kills - a.kills;
      return b.treasureValue - a.treasureValue;
    });
}

export function calculateRoundStandings(players: SimPlayer[]): SimRoundStandingsEntry[] {
  return [...players]
    .map((p) => ({
      playerId: p.id,
      cash: p.cash,
      kills: p.stats.kills,
      treasureValue: p.stats.treasureValue,
      alive: p.alive,
    }))
    .sort((a, b) => {
      if (a.alive !== b.alive) return a.alive ? -1 : 1;
      return b.cash - a.cash;
    });
}
