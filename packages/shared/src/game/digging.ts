import {
  BASE_DIG_RATE_PER_SEC,
  BASE_SOIL_DURABILITY,
} from './constants';
import { EQUIPMENT } from './equipment';
import { tileIndexOf } from './collision';
import type { SimPlayer, SimTileState, SimTreasureEntity } from './types';

export interface DigResult {
  tileOpened: boolean;
  tileIndex: number;
  revealedTreasure: SimTreasureEntity | null;
  digProgressPermille: number;
}

export function getPickaxeMultiplier(player: SimPlayer): number {
  if (player.inventory.upgrades['pickaxe_2'] || player.inventory.items['pickaxe_2']) {
    return EQUIPMENT.pickaxe_2.digMultiplier ?? 1.45;
  }
  return EQUIPMENT.pickaxe_1.digMultiplier ?? 1.0;
}

export function stepDigging(
  player: SimPlayer,
  contactedSoilTile: { x: number; y: number } | null,
  stepMs: number,
  tiles: SimTileState[],
  treasures: SimTreasureEntity[],
): DigResult | null {
  if (!contactedSoilTile) {
    player.digTargetTile = null;
    return null;
  }

  const { x, y } = contactedSoilTile;
  const idx = tileIndexOf(x, y);
  const tile = tiles[idx];
  if (!tile || tile.kind !== 'soil') {
    player.digTargetTile = null;
    return null;
  }

  player.digTargetTile = { x, y };

  const multiplier = getPickaxeMultiplier(player);
  const work = Math.round(BASE_DIG_RATE_PER_SEC * multiplier * (stepMs / 1000));
  tile.durability = Math.max(0, tile.durability - work);

  const digProgressPermille = Math.min(
    1000,
    Math.floor(((BASE_SOIL_DURABILITY - tile.durability) / BASE_SOIL_DURABILITY) * 1000),
  );

  let revealedTreasure: SimTreasureEntity | null = null;
  let tileOpened = false;

  if (tile.durability <= 0) {
    tile.kind = 'floor';
    tileOpened = true;
    player.digTargetTile = null;

    // Check if this tile has an unrevealed treasure
    const hidden = treasures.find((t) => t.tileIndex === idx && !t.revealed && !t.collected);
    if (hidden) {
      hidden.revealed = true;
      revealedTreasure = hidden;
    }
  }

  return {
    tileOpened,
    tileIndex: idx,
    revealedTreasure,
    digProgressPermille,
  };
}
