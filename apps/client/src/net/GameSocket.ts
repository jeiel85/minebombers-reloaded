import {
  PROTOCOL_VERSION,
  type ClientMessage,
  type ServerMessage,
} from '@minebombers/shared';
import { ClockSync } from './ClockSync';
import { Reconnector } from './Reconnector';

export interface GameSocketOptions {
  apiBase: string;
  roomCode: string;
  name: string;
  clientBuild: string;
  resumeToken?: string | null;
  onMessage(message: ServerMessage): void;
  onState?(state: 'connecting' | 'open' | 'closed'): void;
}

export class GameSocket {
  private ws: WebSocket | null = null;
  private inputSeq = 0;
  private actionSeq = 0;
  public clockSync = new ClockSync();
  public reconnector = new Reconnector();
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  public currentResumeToken: string | null = null;
  private isClosedExplicitly = false;

  constructor(private readonly options: GameSocketOptions) {
    this.currentResumeToken = options.resumeToken ?? Reconnector.getResumeToken(options.roomCode);
  }

  connect(): void {
    this.isClosedExplicitly = false;
    this.options.onState?.('connecting');
    const httpUrl = new URL(this.options.apiBase);
    httpUrl.protocol = httpUrl.protocol === 'https:' ? 'wss:' : 'ws:';
    httpUrl.pathname = `/api/rooms/${encodeURIComponent(this.options.roomCode)}/ws`;

    const ws = new WebSocket(httpUrl.toString());
    this.ws = ws;

    ws.addEventListener('open', () => {
      this.options.onState?.('open');
      this.reconnector.reset();

      this.send({
        t: 'c.hello',
        v: PROTOCOL_VERSION,
        name: this.options.name,
        clientBuild: this.options.clientBuild,
        resumeToken: this.currentResumeToken,
      });

      this.startPing();
    });

    ws.addEventListener('message', (event) => {
      if (typeof event.data !== 'string') return;
      try {
        const message = JSON.parse(event.data) as ServerMessage;
        if (message.t === 's.welcome') {
          this.currentResumeToken = message.resumeToken;
          Reconnector.saveResumeToken(this.options.roomCode, message.resumeToken);
        } else if (message.t === 's.pong') {
          this.clockSync.handlePong(message.n, message.serverTime);
        }
        this.options.onMessage(message);
      } catch { /* json parse err */ }
    });

    ws.addEventListener('close', () => {
      this.stopPing();
      this.options.onState?.('closed');
      if (!this.isClosedExplicitly) {
        this.reconnector.scheduleReconnect(() => {
          this.connect();
        });
      }
    });
  }

  setReady(ready: boolean): void {
    this.send({ t: 'c.ready', ready });
  }

  requestStart(): void {
    this.send({ t: 'c.start' });
  }

  sendInput(dx: -1 | 0 | 1, dy: -1 | 0 | 1, slot = 0, primary = false, secondary = false): number {
    const seq = ++this.inputSeq;
    this.send({
      t: 'c.input',
      seq,
      clientTime: Date.now(),
      dx,
      dy,
      primary,
      secondary,
      slot,
    });
    return seq;
  }

  buy(equipmentId: string, quantity = 1): number {
    const seq = ++this.actionSeq;
    this.send({ t: 'c.buy', seq, equipmentId, quantity });
    return seq;
  }

  placeBomb(slot = 0): number {
    const seq = ++this.actionSeq;
    this.send({ t: 'c.action', seq, action: 'place_bomb', slot });
    return seq;
  }

  useItem(slot = 0): number {
    const seq = ++this.actionSeq;
    this.send({ t: 'c.action', seq, action: 'use_item', slot });
    return seq;
  }

  requestResync(): void {
    this.send({ t: 'c.resync', reason: 'client_request', lastServerSeq: 0 });
  }

  close(): void {
    this.isClosedExplicitly = true;
    this.reconnector.cancel();
    this.stopPing();
    this.ws?.close(1000, 'client close');
  }

  private startPing(): void {
    this.stopPing();
    this.pingTimer = setInterval(() => {
      const ping = this.clockSync.createPing();
      this.send({ t: 'c.ping', n: ping.n, sentAt: ping.sentAt });
    }, 5000);
  }

  private stopPing(): void {
    if (this.pingTimer) {
      clearInterval(this.pingTimer);
      this.pingTimer = null;
    }
  }

  private send(message: ClientMessage): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(JSON.stringify(message));
  }
}
