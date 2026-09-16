import { DurableObject } from 'cloudflare:workers';
import {
  DEFAULT_MATCH_ROUNDS,
  DEFAULT_ROUND_DURATION_MS,
  MAX_APP_MESSAGE_BYTES,
  MAX_PLAYERS,
  MIN_PLAYERS,
  PROTOCOL_VERSION,
  STARTING_CASH,
  type ClientMessage,
  type GameEvent,
  type PlayerView,
  type ServerMessage,
  WorldSimulation,
  calculateMatchStandings,
  createDefaultInventory,
  generateClassicMine,
  parseClientMessage,
  processPurchase,
  EQUIPMENT,
  type EquipmentDef,
} from '@minebombers/shared';
import type { Env } from '../index';
import {
  RoomStorage,
  generateResumeToken,
  hashToken,
  type PlayerSlot,
  type RoomRecord,
} from './RoomStorage';
import { SimulationLoop } from './SimulationLoop';
import { SocketRateLimiter } from '../security/RateLimiter';

interface SessionAttachment {
  playerId: string | null;
  roomCode: string;
  name?: string;
  ready?: boolean;
}

export class GameRoom extends DurableObject<Env> {
  private roomCode = '';
  private createdAt = 0;
  private storageHelper: RoomStorage;
  private roomRecord: RoomRecord | null = null;
  private slots: PlayerSlot[] = [];

  private simulation: WorldSimulation | null = null;
  private simLoop: SimulationLoop | null = null;
  private limiters = new Map<WebSocket, SocketRateLimiter>();
  private phaseTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    this.storageHelper = new RoomStorage(this.ctx.storage);
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname === '/init' && request.method === 'POST') {
      const existing = await this.storageHelper.getRoomRecord();
      if (existing) return new Response('Room already exists', { status: 409 });

      this.roomCode = request.headers.get('x-room-code') ?? '';
      this.createdAt = Date.now();
      this.roomRecord = {
        schemaVersion: 1,
        roomCode: this.roomCode,
        createdAt: this.createdAt,
        updatedAt: this.createdAt,
        phase: 'lobby',
        hostPlayerId: null,
        settings: {
          maxPlayers: MAX_PLAYERS,
          totalRounds: DEFAULT_MATCH_ROUNDS,
          roundDurationMs: DEFAULT_ROUND_DURATION_MS,
        },
        matchId: null,
        roundIndex: 1,
      };
      await this.storageHelper.saveRoomRecord(this.roomRecord);
      return new Response(null, { status: 204 });
    }

    await this.ensureHydrated(url.searchParams.get('roomCode') ?? '');

    if (!this.roomRecord) {
      return new Response('Room not found', { status: 404 });
    }

    if (url.pathname === '/status') {
      return new Response(JSON.stringify({
        roomCode: this.roomCode,
        phase: this.roomRecord.phase,
        players: this.connectedCount(),
        reservedSlots: this.slots.length,
        maxPlayers: MAX_PLAYERS,
      }), { headers: { 'content-type': 'application/json' } });
    }

    if (url.pathname !== '/ws' || request.headers.get('Upgrade') !== 'websocket') {
      return new Response('WebSocket upgrade required', { status: 426 });
    }

    const origin = request.headers.get('Origin');
    const allowedOrigins = this.env.ALLOWED_ORIGIN.split(',').map((v) => v.trim()).filter(Boolean);
    if (origin && !allowedOrigins.includes(origin)) {
      return new Response('Origin not allowed', { status: 403 });
    }

    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair) as [WebSocket, WebSocket];

    this.ctx.acceptWebSocket(server);
    server.serializeAttachment({
      playerId: null,
      roomCode: this.roomCode,
    } satisfies SessionAttachment);
    this.limiters.set(server, new SocketRateLimiter());

    return new Response(null, { status: 101, webSocket: client });
  }

  async webSocketMessage(ws: WebSocket, raw: string | ArrayBuffer): Promise<void> {
    await this.ensureHydrated('');

    if (typeof raw !== 'string') {
      ws.close(1003, 'text messages only');
      return;
    }

    if (new TextEncoder().encode(raw).byteLength > MAX_APP_MESSAGE_BYTES) {
      ws.close(1009, 'message too large');
      return;
    }

    let msg: ClientMessage;
    try {
      msg = parseClientMessage(raw);
    } catch {
      this.send(ws, { t: 's.error', code: 'INVALID_MESSAGE', message: 'Invalid message.' });
      return;
    }

    let attachment = ws.deserializeAttachment() as SessionAttachment;

    if (msg.t === 'c.hello') {
      await this.handleHello(ws, attachment, msg);
      return;
    }

    if (!attachment.playerId) {
      this.send(ws, { t: 's.error', code: 'HELLO_REQUIRED', message: 'Send hello first.' });
      return;
    }

    const slot = this.slots.find((s) => s.playerId === attachment.playerId);
    if (!slot) {
      this.send(ws, { t: 's.error', code: 'SESSION_NOT_FOUND', message: 'Session not found.' });
      return;
    }

    const limiter = this.limiters.get(ws) ?? new SocketRateLimiter();

    switch (msg.t) {
      case 'c.ready':
        slot.ready = msg.ready;
        attachment = { ...attachment, ready: msg.ready };
        ws.serializeAttachment(attachment);
        this.broadcastRoster();

        // In shop phase, if all connected players ready, can advance immediately
        if (this.roomRecord?.phase === 'shop') {
          const connectedSlots = this.slots.filter((s) => s.connected);
          if (connectedSlots.length > 0 && connectedSlots.every((s) => s.ready)) {
            this.clearPhaseTimer();
            this.startCountdown();
          }
        }
        break;

      case 'c.start':
        if (this.roomRecord?.phase !== 'lobby') {
          this.send(ws, { t: 's.error', code: 'ILLEGAL_START', message: 'Match already started.' });
          return;
        }
        if (this.roomRecord.hostPlayerId !== slot.playerId) {
          this.send(ws, { t: 's.error', code: 'NOT_HOST', message: 'Only the host can start.' });
          return;
        }
        const connectedPlayers = this.slots.filter((s) => s.connected);
        if (connectedPlayers.length < MIN_PLAYERS) {
          this.send(ws, { t: 's.error', code: 'NOT_ENOUGH_PLAYERS', message: `Need at least ${MIN_PLAYERS} players.` });
          return;
        }
        this.startMatch();
        break;

      case 'c.buy':
        if (this.roomRecord?.phase !== 'shop') {
          this.send(ws, { t: 's.error', code: 'NOT_SHOP_PHASE', message: 'Shop is currently closed.' });
          return;
        }
        if (!limiter.checkBuy()) {
          this.send(ws, { t: 's.error', code: 'RATE_LIMIT_EXCEEDED', message: 'Too many buy requests.' });
          return;
        }
        const tempPlayer = {
          id: slot.playerId,
          name: slot.displayName,
          x: 0,
          y: 0,
          hp: 100,
          alive: true,
          cash: slot.cash,
          inventory: slot.inventory,
          input: { dx: 0 as const, dy: 0 as const, seq: 0, clientTime: 0, lastReceivedAt: 0 },
          invulnerableUntil: 0,
          stats: slot.stats,
          digTargetTile: null,
        };
        const buyResult = processPurchase(tempPlayer, msg.equipmentId, msg.quantity);
        slot.cash = buyResult.cash;
        slot.inventory = buyResult.inventory;
        await this.storageHelper.savePlayerSlots(this.slots);

        this.send(ws, {
          t: 's.purchase_result',
          requestSeq: msg.seq,
          accepted: buyResult.accepted,
          code: buyResult.code,
          cash: slot.cash,
          inventory: slot.inventory,
        });
        break;

      case 'c.input':
        if (this.roomRecord?.phase === 'playing' && this.simulation) {
          if (!limiter.checkMove()) return;
          this.simulation.setPlayerInput(
            slot.playerId,
            msg.dx,
            msg.dy,
            msg.seq,
            msg.clientTime,
            this.simulation.simTime,
            msg.primary,
            msg.secondary,
          );
        }
        break;

      case 'c.action':
        if (this.roomRecord?.phase === 'playing' && this.simulation) {
          if (!limiter.checkAction()) return;
          this.simulation.queueAction(slot.playerId, msg.action, msg.slot, msg.seq);
        }
        break;

      case 'c.ping':
        this.send(ws, { t: 's.pong', n: msg.n, serverTime: Date.now() });
        break;

      case 'c.resync':
        if (this.simulation && this.roomRecord?.phase === 'playing') {
          const snapshot = this.simulation.getSnapshot(slot.playerId);
          this.send(ws, snapshot);
        }
        break;
    }
  }

  async webSocketClose(ws: WebSocket): Promise<void> {
    await this.ensureHydrated('');
    const attachment = ws.deserializeAttachment() as SessionAttachment;
    this.limiters.delete(ws);
    if (!attachment.playerId) return;

    const slot = this.slots.find((s) => s.playerId === attachment.playerId);
    if (!slot) return;

    slot.connected = false;
    slot.reservedUntil = Date.now() + 20_000; // 20-second grace window

    // Neutralize movement input in active simulation
    if (this.simulation) {
      const p = this.simulation.players.find((pl) => pl.id === slot.playerId);
      if (p) {
        p.input.dx = 0;
        p.input.dy = 0;
      }
    }

    await this.storageHelper.savePlayerSlots(this.slots);
    this.broadcastRoster();

    // Schedule 20-second grace expiry check
    setTimeout(async () => {
      await this.checkGraceExpiry(slot.playerId);
    }, 20_500);
  }

  private async checkGraceExpiry(playerId: string): Promise<void> {
    const slot = this.slots.find((s) => s.playerId === playerId);
    if (!slot || slot.connected) return;

    if (slot.reservedUntil && Date.now() >= slot.reservedUntil) {
      if (this.roomRecord?.phase === 'lobby') {
        // In lobby, remove expired slot
        this.slots = this.slots.filter((s) => s.playerId !== playerId);
      }
      this.recomputeHost();
      await this.storageHelper.savePlayerSlots(this.slots);
      if (this.roomRecord) await this.storageHelper.saveRoomRecord(this.roomRecord);
      this.broadcastRoster();
    }
  }

  private async ensureHydrated(roomCodeHint: string): Promise<void> {
    if (!this.roomRecord) {
      this.roomRecord = await this.storageHelper.getRoomRecord();
      if (this.roomRecord) {
        this.roomCode = this.roomRecord.roomCode;
        this.createdAt = this.roomRecord.createdAt;
      } else if (roomCodeHint) {
        this.roomCode = roomCodeHint;
      }
    }
    if (this.slots.length === 0) {
      this.slots = await this.storageHelper.getPlayerSlots();
    }
  }

  private async handleHello(
    ws: WebSocket,
    attachment: SessionAttachment,
    msg: Extract<ClientMessage, { t: 'c.hello' }>,
  ): Promise<void> {
    if (attachment.playerId) {
      this.send(ws, { t: 's.error', code: 'HELLO_ALREADY_SENT', message: 'Hello already received.' });
      return;
    }

    if (msg.v !== PROTOCOL_VERSION) {
      this.send(ws, { t: 's.error', code: 'VERSION_MISMATCH', message: 'Refresh to update the game.' });
      ws.close(1008, 'version mismatch');
      return;
    }

    // Check if client provided a resumeToken
    if (msg.resumeToken) {
      const tokenHash = await hashToken(msg.resumeToken);
      const existingSlot = this.slots.find((s) => s.resumeTokenHash === tokenHash);

      if (existingSlot && (existingSlot.reservedUntil === null || Date.now() <= existingSlot.reservedUntil)) {
        // Successful reconnect!
        existingSlot.connected = true;
        existingSlot.reservedUntil = null;

        // Rotate resume token
        const newResumeToken = generateResumeToken();
        existingSlot.resumeTokenHash = await hashToken(newResumeToken);
        await this.storageHelper.savePlayerSlots(this.slots);

        ws.serializeAttachment({
          playerId: existingSlot.playerId,
          roomCode: this.roomCode,
          name: existingSlot.displayName,
          ready: existingSlot.ready,
        } satisfies SessionAttachment);

        this.send(ws, {
          t: 's.welcome',
          v: PROTOCOL_VERSION,
          playerId: existingSlot.playerId,
          resumeToken: newResumeToken,
          room: {
            code: this.roomCode,
            phase: this.roomRecord?.phase ?? 'lobby',
            hostPlayerId: this.roomRecord?.hostPlayerId ?? existingSlot.playerId,
            players: this.playerViews(),
          },
        });

        this.broadcastRoster();

        // Resync current game state if in active match
        if (this.simulation && this.roomRecord?.phase === 'playing') {
          const snap = this.simulation.getSnapshot(existingSlot.playerId);
          this.send(ws, snap);
        }
        return;
      }
    }

    // New Join: Check capacity (active + reserved <= MAX_PLAYERS)
    if (this.slots.length >= MAX_PLAYERS) {
      this.send(ws, { t: 's.error', code: 'ROOM_FULL', message: 'Room is full.' });
      ws.close(1008, 'room full');
      return;
    }

    const normalizedName = Array.from(msg.name.normalize('NFKC').trim()).slice(0, 16).join('');
    if (!normalizedName) {
      this.send(ws, { t: 's.error', code: 'INVALID_NAME', message: 'Display name is required.' });
      return;
    }

    const playerId = crypto.randomUUID();
    const resumeToken = generateResumeToken();
    const tokenHash = await hashToken(resumeToken);
    const joinedAt = Date.now();

    const newSlot: PlayerSlot = {
      playerId,
      displayName: normalizedName,
      joinedAt,
      connected: true,
      ready: false,
      reservedUntil: null,
      resumeTokenHash: tokenHash,
      cash: STARTING_CASH,
      inventory: createDefaultInventory(),
      stats: { kills: 0, treasureValue: 0, roundWins: 0 },
    };

    this.slots.push(newSlot);
    if (!this.roomRecord?.hostPlayerId) {
      this.roomRecord!.hostPlayerId = playerId;
    }

    await this.storageHelper.savePlayerSlots(this.slots);
    if (this.roomRecord) await this.storageHelper.saveRoomRecord(this.roomRecord);

    ws.serializeAttachment({
      playerId,
      roomCode: this.roomCode,
      name: normalizedName,
      ready: false,
    } satisfies SessionAttachment);

    this.send(ws, {
      t: 's.welcome',
      v: PROTOCOL_VERSION,
      playerId,
      resumeToken,
      room: {
        code: this.roomCode,
        phase: this.roomRecord?.phase ?? 'lobby',
        hostPlayerId: this.roomRecord?.hostPlayerId ?? playerId,
        players: this.playerViews(),
      },
    });

    this.broadcastRoster();
  }

  private startMatch(): void {
    if (!this.roomRecord) return;
    this.roomRecord.phase = 'shop';
    this.roomRecord.matchId = `m_${Date.now()}`;
    this.roomRecord.roundIndex = 1;
    this.enterShop();
  }

  private enterShop(): void {
    if (!this.roomRecord) return;
    this.roomRecord.phase = 'shop';
    // Reset ready states for shop
    for (const slot of this.slots) {
      slot.ready = false;
    }

    const shopDuration = 20_000;
    const closesAt = Date.now() + shopDuration;

    // Send s.shop to each player with their own cash & inventory
    const offers = (Object.values(EQUIPMENT) as EquipmentDef[])
      .filter((eq) => eq.cost > 0)
      .map((eq) => ({
        equipmentId: eq.id,
        price: eq.cost,
        maxOwned: eq.maxOwned,
      }));

    for (const ws of this.ctx.getWebSockets()) {
      const a = ws.deserializeAttachment() as SessionAttachment | null;
      if (!a?.playerId) continue;
      const slot = this.slots.find((s) => s.playerId === a.playerId);
      if (!slot) continue;

      this.send(ws, {
        t: 's.shop',
        closesAt,
        cash: slot.cash,
        inventory: slot.inventory,
        offers,
      });
    }

    this.broadcastRoster();

    this.clearPhaseTimer();
    this.phaseTimer = setTimeout(() => {
      this.startCountdown();
    }, shopDuration);
  }

  private startCountdown(): void {
    if (!this.roomRecord) return;
    this.roomRecord.phase = 'countdown';

    const seed = (crypto.getRandomValues(new Uint32Array(1))[0] ?? 42) >>> 0;
    const map = generateClassicMine(seed, Math.max(MIN_PLAYERS, this.slots.length));

    const simPlayers = this.slots.map((s) => ({
      id: s.playerId,
      name: s.displayName,
      cash: s.cash,
      inventory: s.inventory,
      stats: s.stats,
    }));

    this.simulation = new WorldSimulation(
      seed,
      map,
      simPlayers,
      this.roomRecord.roundIndex,
      this.roomRecord.settings.roundDurationMs,
    );

    const startsAt = Date.now() + 3000;
    const endsAt = startsAt + this.roomRecord.settings.roundDurationMs;

    this.broadcast({
      t: 's.start',
      matchId: this.roomRecord.matchId ?? 'm_1',
      roundIndex: this.roomRecord.roundIndex,
      seed,
      startsAt,
      endsAt,
      mapGenerator: 'classic-mine-v1',
    });

    this.clearPhaseTimer();
    this.phaseTimer = setTimeout(() => {
      this.enterPlaying();
    }, 3000);
  }

  private enterPlaying(): void {
    if (!this.roomRecord || !this.simulation) return;
    this.roomRecord.phase = 'playing';

    this.simLoop = new SimulationLoop(this.simulation, {
      onEvents: (events: GameEvent[]) => {
        this.broadcast({
          t: 's.event',
          serverSeq: this.simulation!.serverSeq,
          events,
        });
      },
      onSnapshot: () => {
        for (const ws of this.ctx.getWebSockets()) {
          const a = ws.deserializeAttachment() as SessionAttachment | null;
          const playerId = a?.playerId ?? undefined;
          const snap = this.simulation!.getSnapshot(playerId);
          this.send(ws, snap);
        }
      },
      onCheckpoint: async () => {
        if (this.simulation) {
          const cp = this.simulation.createCheckpoint();
          await this.storageHelper.saveCheckpoint(cp);
        }
      },
      onRoundEnded: (reason: string) => {
        this.handleRoundEnded(reason);
      },
    });

    this.simLoop.start();
  }

  private handleRoundEnded(reason: string): void {
    if (!this.roomRecord || !this.simulation) return;
    this.simLoop?.stop();
    this.roomRecord.phase = 'round_result';

    // Synchronize player cash and stats back to persistent slots
    for (const simP of this.simulation.players) {
      const slot = this.slots.find((s) => s.playerId === simP.id);
      if (slot) {
        slot.cash = simP.cash;
        slot.stats = { ...simP.stats };
        slot.inventory = { ...simP.inventory };
      }
    }
    this.storageHelper.savePlayerSlots(this.slots);

    const standings = this.simulation.getRoundStandings();
    this.broadcast({
      t: 's.round_result',
      roundIndex: this.roomRecord.roundIndex,
      standings,
    });

    this.clearPhaseTimer();
    this.phaseTimer = setTimeout(() => {
      if (this.roomRecord && this.roomRecord.roundIndex >= this.roomRecord.settings.totalRounds) {
        this.enterMatchResult();
      } else if (this.roomRecord) {
        this.roomRecord.roundIndex++;
        this.enterShop();
      }
    }, 8000);
  }

  private enterMatchResult(): void {
    if (!this.roomRecord) return;
    this.roomRecord.phase = 'match_result';

    const tempSimPlayers = this.slots.map((s) => ({
      id: s.playerId,
      name: s.displayName,
      x: 0,
      y: 0,
      hp: 100,
      alive: true,
      cash: s.cash,
      inventory: s.inventory,
      input: { dx: 0 as const, dy: 0 as const, seq: 0, clientTime: 0, lastReceivedAt: 0 },
      invulnerableUntil: 0,
      stats: s.stats,
      digTargetTile: null,
    }));

    const standings = calculateMatchStandings(tempSimPlayers);
    this.broadcast({
      t: 's.match_result',
      standings,
    });
  }

  private clearPhaseTimer(): void {
    if (this.phaseTimer) {
      clearTimeout(this.phaseTimer);
      this.phaseTimer = null;
    }
  }

  private recomputeHost(): void {
    const connected = [...this.slots]
      .filter((s) => s.connected)
      .sort((a, b) => a.joinedAt - b.joinedAt);
    if (this.roomRecord) {
      this.roomRecord.hostPlayerId = connected[0]?.playerId ?? null;
    }
  }

  private connectedCount(): number {
    return this.slots.filter((s) => s.connected).length;
  }

  private playerViews(): PlayerView[] {
    return this.slots.map((s) => ({
      id: s.playerId,
      name: s.displayName,
      ready: s.ready,
      connected: s.connected,
      cash: s.cash,
    }));
  }

  private broadcastRoster(): void {
    const message: ServerMessage = {
      t: 's.roster',
      hostPlayerId: this.roomRecord?.hostPlayerId ?? '',
      players: this.playerViews(),
    };
    this.broadcast(message);
  }

  private broadcast(message: ServerMessage): void {
    const text = JSON.stringify(message);
    for (const socket of this.ctx.getWebSockets()) {
      try { socket.send(text); } catch { /* connection closing */ }
    }
  }

  private send(ws: WebSocket, message: ServerMessage): void {
    try {
      ws.send(JSON.stringify(message));
    } catch { /* connection closing */ }
  }
}
