import { PROTOCOL_VERSION } from './config';

export type RoomPhase =
  | 'lobby'
  | 'shop'
  | 'countdown'
  | 'playing'
  | 'round_result'
  | 'match_result'
  | 'closed';

export type TileKind = 'floor' | 'soil' | 'rock' | 'hazard' | 'spawn';

export interface InventoryState {
  selectedSlot: number;
  items: Record<string, number>;
  upgrades: Record<string, number>;
}

export interface PlayerView {
  id: string;
  name: string;
  ready: boolean;
  connected: boolean;
  x?: number;
  y?: number;
  hp?: number;
  alive?: boolean;
  cash?: number;
}

export interface ExplosiveView {
  kind: 'explosive';
  id: string;
  ownerId: string;
  definitionId: string;
  tileX: number;
  tileY: number;
  explodeAt: number;
  isRemote?: boolean;
}

export interface TreasureView {
  kind: 'treasure';
  id: string;
  tileX: number;
  tileY: number;
  value: number;
  rarity: 'basic' | 'rare' | 'silver' | 'ruby' | 'diamond' | 'chest';
}

export interface PickupView {
  kind: 'pickup';
  id: string;
  tileX: number;
  tileY: number;
  definitionId: string;
}

export interface ProjectileView {
  kind: 'projectile';
  id: string;
  definitionId: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
}

export interface MonsterView {
  kind: 'monster';
  id: string;
  monsterKind: 'slime' | 'bat';
  x: number;
  y: number;
  hp: number;
}

export interface FallingRockView {
  kind: 'falling_rock';
  id: string;
  tileX: number;
  tileY: number;
}

export type EntityView =
  | ExplosiveView
  | TreasureView
  | PickupView
  | ProjectileView
  | MonsterView
  | FallingRockView;

export interface TileDelta {
  index: number;
  kind: TileKind;
  digProgressPermille?: number;
}

export type GameEvent =
  | {
      kind: 'explosive_placed';
      id: string;
      ownerId: string;
      definitionId: string;
      tileX: number;
      tileY: number;
      explodeAt: number;
    }
  | {
      kind: 'explosion';
      id: string;
      sourceId: string;
      cells: Array<{ x: number; y: number }>;
      at: number;
    }
  | { kind: 'tile_changed'; index: number; tile: TileKind }
  | { kind: 'treasure_collected'; playerId: string; entityId: string; value: number }
  | { kind: 'pickup_collected'; playerId: string; entityId: string; definitionId: string }
  | { kind: 'damage'; playerId: string; sourceId: string; amount: number; hpAfter: number }
  | { kind: 'player_eliminated'; playerId: string; killerPlayerId: string | null }
  | { kind: 'cash_changed'; playerId: string; cash: number; reason: string }
  | { kind: 'phase_changed'; phase: RoomPhase; at: number }
  | { kind: 'projectile_fired'; id: string; ownerId: string; x: number; y: number; vx: number; vy: number }
  | { kind: 'flame_burst'; playerId: string; cells: Array<{ x: number; y: number }> }
  | { kind: 'mine_collapse'; fallingRocks: Array<{ tileX: number; tileY: number }> }
  | { kind: 'teleported'; playerId: string; tileX: number; tileY: number }
  | {
      kind: 'round_ended';
      roundIndex: number;
      reason: 'timeout' | 'treasure_empty' | 'last_survivor' | 'admin';
    };

export type ClientMessage =
  | {
      t: 'c.hello';
      v: typeof PROTOCOL_VERSION;
      name: string;
      clientBuild: string;
      resumeToken?: string | null;
    }
  | { t: 'c.ready'; ready: boolean }
  | { t: 'c.start' }
  | {
      t: 'c.input';
      seq: number;
      clientTime: number;
      dx: -1 | 0 | 1;
      dy: -1 | 0 | 1;
      primary: boolean;
      secondary: boolean;
      slot: number;
    }
  | {
      t: 'c.action';
      seq: number;
      action: 'place_bomb' | 'use_secondary' | 'interact' | 'use_item';
      slot?: number;
    }
  | {
      t: 'c.buy';
      seq: number;
      equipmentId: string;
      quantity: number;
    }
  | { t: 'c.ping'; n: number; sentAt: number }
  | { t: 'c.resync'; reason: string; lastServerSeq: number };

export type ServerMessage =
  | {
      t: 's.welcome';
      v: typeof PROTOCOL_VERSION;
      playerId: string;
      resumeToken: string;
      room: {
        code: string;
        phase: RoomPhase;
        hostPlayerId: string;
        players: PlayerView[];
      };
    }
  | { t: 's.roster'; hostPlayerId: string; players: PlayerView[] }
  | {
      t: 's.shop';
      closesAt: number;
      cash: number;
      inventory: InventoryState;
      offers: Array<{
        equipmentId: string;
        price: number;
        maxOwned?: number;
      }>;
    }
  | {
      t: 's.purchase_result';
      requestSeq: number;
      accepted: boolean;
      code?: string;
      cash: number;
      inventory: InventoryState;
    }
  | {
      t: 's.start';
      matchId: string;
      roundIndex: number;
      seed: number;
      startsAt: number;
      endsAt: number;
      mapGenerator: string;
    }
  | {
      t: 's.snapshot';
      serverSeq: number;
      serverTime: number;
      ackInputSeq: number;
      phase: RoomPhase;
      roundIndex: number;
      roundEndsAt: number | null;
      players: Array<{
        id: string;
        x: number;
        y: number;
        hp: number;
        alive: boolean;
        cash: number;
      }>;
      entities: EntityView[];
      changedTiles: TileDelta[];
    }
  | { t: 's.event'; serverSeq: number; events: GameEvent[] }
  | {
      t: 's.round_result';
      roundIndex: number;
      standings: Array<{
        playerId: string;
        cash: number;
        kills: number;
        treasureValue: number;
        alive: boolean;
      }>;
    }
  | {
      t: 's.match_result';
      standings: Array<{
        playerId: string;
        cash: number;
        roundWins: number;
        kills: number;
        treasureValue: number;
      }>;
    }
  | { t: 's.pong'; n: number; serverTime: number }
  | { t: 's.error'; code: string; message: string };

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isIntegerIn(value: unknown, min: number, max: number): value is number {
  return Number.isInteger(value) && (value as number) >= min && (value as number) <= max;
}

function isSeq(value: unknown): value is number {
  return isIntegerIn(value, 0, Number.MAX_SAFE_INTEGER);
}

export function parseClientMessage(text: string): ClientMessage {
  let value: unknown;
  try {
    value = JSON.parse(text) as unknown;
  } catch {
    throw new Error('INVALID_JSON');
  }

  if (!isObject(value) || typeof value.t !== 'string') {
    throw new Error('INVALID_MESSAGE');
  }

  switch (value.t) {
    case 'c.hello': {
      if (
        value.v !== PROTOCOL_VERSION ||
        typeof value.name !== 'string' ||
        value.name.length < 1 ||
        value.name.length > 32 ||
        typeof value.clientBuild !== 'string' ||
        value.clientBuild.length < 1 ||
        value.clientBuild.length > 64 ||
        !(
          value.resumeToken === undefined ||
          value.resumeToken === null ||
          (typeof value.resumeToken === 'string' && value.resumeToken.length <= 256)
        )
      ) throw new Error('INVALID_HELLO');
      return value as unknown as ClientMessage;
    }
    case 'c.ready':
      if (typeof value.ready !== 'boolean') throw new Error('INVALID_READY');
      return value as unknown as ClientMessage;
    case 'c.start':
      return { t: 'c.start' };
    case 'c.input':
      if (
        !isSeq(value.seq) ||
        !Number.isFinite(value.clientTime) ||
        ![-1, 0, 1].includes(value.dx as number) ||
        ![-1, 0, 1].includes(value.dy as number) ||
        typeof value.primary !== 'boolean' ||
        typeof value.secondary !== 'boolean' ||
        !isIntegerIn(value.slot, 0, 3)
      ) throw new Error('INVALID_INPUT');
      return value as unknown as ClientMessage;
    case 'c.action':
      if (
        !isSeq(value.seq) ||
        !['place_bomb', 'use_secondary', 'interact', 'use_item'].includes(String(value.action)) ||
        !(value.slot === undefined || isIntegerIn(value.slot, 0, 3))
      ) throw new Error('INVALID_ACTION');
      return value as unknown as ClientMessage;
    case 'c.buy':
      if (
        !isSeq(value.seq) ||
        typeof value.equipmentId !== 'string' ||
        !/^[a-z0-9_.-]{1,48}$/.test(value.equipmentId) ||
        !isIntegerIn(value.quantity, 1, 99)
      ) throw new Error('INVALID_BUY');
      return value as unknown as ClientMessage;
    case 'c.ping':
      if (!isSeq(value.n) || !Number.isFinite(value.sentAt)) throw new Error('INVALID_PING');
      return value as unknown as ClientMessage;
    case 'c.resync':
      if (
        typeof value.reason !== 'string' ||
        value.reason.length > 64 ||
        !isSeq(value.lastServerSeq)
      ) throw new Error('INVALID_RESYNC');
      return value as unknown as ClientMessage;
    default:
      throw new Error('UNKNOWN_MESSAGE_TYPE');
  }
}
