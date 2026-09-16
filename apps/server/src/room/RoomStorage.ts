import type {
  InventoryState,
  RoomPhase,
  SimPlayerStats,
  SimWorldCheckpoint,
} from '@minebombers/shared';

export interface RoomSettings {
  maxPlayers: number;
  totalRounds: number;
  roundDurationMs: number;
}

export interface RoomRecord {
  schemaVersion: 1;
  roomCode: string;
  createdAt: number;
  updatedAt: number;
  phase: RoomPhase;
  hostPlayerId: string | null;
  settings: RoomSettings;
  matchId: string | null;
  roundIndex: number;
}

export interface PlayerSlot {
  playerId: string;
  displayName: string;
  joinedAt: number;
  connected: boolean;
  ready: boolean;
  reservedUntil: number | null;
  resumeTokenHash: string;
  cash: number;
  inventory: InventoryState;
  stats: SimPlayerStats;
}

export async function hashToken(token: string): Promise<string> {
  const data = new TextEncoder().encode(token);
  const hash = await crypto.subtle.digest('SHA-256', data);
  return Array.from(new Uint8Array(hash), (b) => b.toString(16).padStart(2, '0')).join('');
}

export function generateResumeToken(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(24));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

export class RoomStorage {
  constructor(private readonly storage: DurableObjectStorage) {}

  public async getRoomRecord(): Promise<RoomRecord | null> {
    const record = await this.storage.get<RoomRecord>('room');
    return record ?? null;
  }

  public async saveRoomRecord(record: RoomRecord): Promise<void> {
    record.updatedAt = Date.now();
    await this.storage.put('room', record);
  }

  public async getPlayerSlots(): Promise<PlayerSlot[]> {
    const slots = await this.storage.get<PlayerSlot[]>('players');
    return slots ?? [];
  }

  public async savePlayerSlots(slots: PlayerSlot[]): Promise<void> {
    await this.storage.put('players', slots);
  }

  public async getCheckpoint(): Promise<SimWorldCheckpoint | null> {
    const cp = await this.storage.get<SimWorldCheckpoint>('checkpoint');
    return cp ?? null;
  }

  public async saveCheckpoint(cp: SimWorldCheckpoint | null): Promise<void> {
    if (cp === null) {
      await this.storage.delete('checkpoint');
    } else {
      await this.storage.put('checkpoint', cp);
    }
  }
}
