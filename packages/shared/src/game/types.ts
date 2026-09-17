import type { InventoryState, RoomPhase, TileKind } from '../protocol';

export interface SimPlayerStats {
  kills: number;
  treasureValue: number;
  roundWins: number;
}

export interface SimPlayerInput {
  dx: -1 | 0 | 1;
  dy: -1 | 0 | 1;
  seq: number;
  clientTime: number;
  lastReceivedAt: number;
  primary?: boolean;
  secondary?: boolean;
}

export interface SimPlayer {
  id: string;
  name: string;
  x: number; // in world units (1024 / tile)
  y: number;
  hp: number;
  alive: boolean;
  cash: number;
  inventory: InventoryState;
  input: SimPlayerInput;
  invulnerableUntil: number;
  stats: SimPlayerStats;
  digTargetTile: { x: number; y: number } | null;
}

export interface SimExplosive {
  id: string;
  ownerId: string;
  definitionId: string;
  tileX: number;
  tileY: number;
  explodeAt: number;
  radius: number;
  damage: number;
  penetration: number;
  isRemote?: boolean;
}

export interface SimMine {
  id: string;
  ownerId: string;
  definitionId: string;
  tileX: number;
  tileY: number;
  armedAt: number;
  triggerRadiusUnits: number;
  radius: number;
  damage: number;
  detonated?: boolean;
}

export interface SimProjectile {
  id: string;
  ownerId: string;
  definitionId: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  damage: number;
  blastRadiusTiles: number;
  active: boolean;
}

export interface SimMonster {
  id: string;
  kind: 'slime' | 'bat';
  x: number;
  y: number;
  hp: number;
  maxHp: number;
  moveTimerMs: number;
  dx: -1 | 0 | 1;
  dy: -1 | 0 | 1;
  alive: boolean;
  bounty: number;
}

export interface SimFallingRock {
  id: string;
  tileX: number;
  tileY: number;
  impactTime: number;
  dropped: boolean;
}

export interface SimTreasureEntity {
  id: string;
  tileIndex: number;
  tileX: number;
  tileY: number;
  rarity: 'basic' | 'rare' | 'silver' | 'ruby' | 'diamond' | 'chest';
  value: number;
  revealed: boolean;
  collected: boolean;
}

export interface SimPickupItem {
  id: string;
  tileX: number;
  tileY: number;
  definitionId: 'ammo' | 'med_kit' | 'dynamite' | 'rocket';
  collected: boolean;
}

export interface SimTileState {
  kind: TileKind;
  durability: number; // remaining dig work (0 to BASE_SOIL_DURABILITY)
}

export interface SimRoundStandingsEntry {
  playerId: string;
  cash: number;
  kills: number;
  treasureValue: number;
  alive: boolean;
}

export interface SimMatchStandingsEntry {
  playerId: string;
  cash: number;
  roundWins: number;
  kills: number;
  treasureValue: number;
}

export interface SimWorldCheckpoint {
  roundIndex: number;
  simTime: number;
  roundEndsAt: number;
  seed: number;
  players: SimPlayer[];
  explosives: SimExplosive[];
  mines: SimMine[];
  treasures: SimTreasureEntity[];
  tiles: SimTileState[];
}
