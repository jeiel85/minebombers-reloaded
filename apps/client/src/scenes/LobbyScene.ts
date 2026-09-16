import Phaser from 'phaser';
import type { GameSocket } from '../net/GameSocket';
import type { PlayerView, ServerMessage } from '@minebombers/shared';
import { AssetRegistry } from '../game/AssetRegistry';

export interface LobbySceneData {
  roomCode: string;
  displayName: string;
  apiBase: string;
  socket: GameSocket;
  isHost: boolean;
}

export class LobbyScene extends Phaser.Scene {
  private dataPayload!: LobbySceneData;
  private socket!: GameSocket;
  private myPlayerId = '';
  private hostPlayerId = '';
  private players: PlayerView[] = [];
  private isReady = false;

  private playerListContainer!: Phaser.GameObjects.Container;
  private readyBtn!: Phaser.GameObjects.Text;
  private startBtn!: Phaser.GameObjects.Text;
  private statusText!: Phaser.GameObjects.Text;

  constructor() {
    super('lobby');
  }

  init(data: LobbySceneData): void {
    this.dataPayload = data;
    this.socket = data.socket;
  }

  create(): void {
    const width = this.cameras.main.width;
    const height = this.cameras.main.height;

    // Title
    this.add.text(width / 2, 50, 'GAME LOBBY', {
      fontFamily: 'monospace',
      fontSize: '32px',
      fontStyle: 'bold',
      color: '#f39c12',
    }).setOrigin(0.5);

    // Room Code with Copy action
    const codeText = this.add.text(width / 2, 100, `ROOM CODE: ${this.dataPayload.roomCode} [📋 Copy]`, {
      fontFamily: 'monospace',
      fontSize: '20px',
      color: '#00ffcc',
      backgroundColor: '#1a1a1a',
      padding: { x: 16, y: 8 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    codeText.on('pointerdown', async () => {
      try {
        await navigator.clipboard.writeText(this.dataPayload.roomCode);
        codeText.setText(`ROOM CODE: ${this.dataPayload.roomCode} [COPIED! ✅]`);
        setTimeout(() => {
          codeText.setText(`ROOM CODE: ${this.dataPayload.roomCode} [📋 Copy]`);
        }, 2000);
      } catch { /* clipboard permission */ }
    });

    this.statusText = this.add.text(width / 2, 145, 'Connecting to room...', {
      fontFamily: 'monospace',
      fontSize: '15px',
      color: '#bdc3c7',
    }).setOrigin(0.5);

    // Players list container
    this.playerListContainer = this.add.container(width / 2, 280);

    // Action Buttons
    this.readyBtn = this.add.text(width / 2 - 120, height - 80, 'READY', {
      fontFamily: 'monospace',
      fontSize: '20px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: '#27ae60',
      padding: { x: 28, y: 12 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    this.readyBtn.on('pointerdown', () => {
      this.isReady = !this.isReady;
      this.readyBtn.setText(this.isReady ? 'NOT READY' : 'READY');
      this.readyBtn.setBackgroundColor(this.isReady ? '#e74c3c' : '#27ae60');
      this.socket.setReady(this.isReady);
    });

    this.startBtn = this.add.text(width / 2 + 120, height - 80, 'START GAME', {
      fontFamily: 'monospace',
      fontSize: '20px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: '#2980b9',
      padding: { x: 28, y: 12 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    this.startBtn.setVisible(false);
    this.startBtn.on('pointerdown', () => {
      this.socket.requestStart();
    });

    // Leave button
    const leaveBtn = this.add.text(60, 40, '← LEAVE', {
      fontFamily: 'monospace',
      fontSize: '16px',
      color: '#e74c3c',
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    leaveBtn.on('pointerdown', () => {
      this.socket.close();
      this.scene.start('menu');
    });

    // Setup socket listeners
    this.setupSocket();
  }

  private setupSocket(): void {
    // Override socket message handler
    (this.socket as unknown as { options: { onMessage: (msg: ServerMessage) => void } }).options.onMessage = (msg: ServerMessage) => {
      this.handleServerMessage(msg);
    };

    this.socket.connect();
  }

  private handleServerMessage(msg: ServerMessage): void {
    switch (msg.t) {
      case 's.welcome':
        this.myPlayerId = msg.playerId;
        this.hostPlayerId = msg.room.hostPlayerId;
        this.players = msg.room.players;
        this.statusText.setText(`Connected! (${this.players.length}/8 players)`);
        this.updateRosterUI();
        break;

      case 's.roster':
        this.hostPlayerId = msg.hostPlayerId;
        this.players = msg.players;
        this.statusText.setText(`Waiting for players... (${this.players.length}/8)`);
        this.updateRosterUI();
        break;

      case 's.shop':
        this.scene.start('shop', {
          ...this.dataPayload,
          myPlayerId: this.myPlayerId,
          shopData: msg,
        });
        break;

      case 's.start':
        this.scene.start('game', {
          mode: 'multiplayer',
          ...this.dataPayload,
          myPlayerId: this.myPlayerId,
          startData: msg,
        });
        break;

      case 's.error':
        this.statusText.setText(`Error: ${msg.message}`);
        this.statusText.setColor('#e74c3c');
        break;
    }
  }

  private updateRosterUI(): void {
    this.playerListContainer.removeAll(true);

    const isHost = this.myPlayerId === this.hostPlayerId;
    this.startBtn.setVisible(isHost);

    this.players.forEach((p, index) => {
      const yOffset = (index - this.players.length / 2) * 44;
      const color = AssetRegistry.getPlayerColor(index);
      const isMe = p.id === this.myPlayerId;
      const isRoomHost = p.id === this.hostPlayerId;

      const bg = this.add.rectangle(0, yOffset, 420, 38, 0x1f242d, 0.9);
      const dot = this.add.circle(-180, yOffset, 8, color);

      let nameLabel = p.name;
      if (isMe) nameLabel += ' (You)';
      if (isRoomHost) nameLabel += ' 👑 HOST';

      const name = this.add.text(-160, yOffset - 8, nameLabel, {
        fontFamily: 'monospace',
        fontSize: '16px',
        color: isMe ? '#00ffcc' : '#ffffff',
      });

      const readyStatus = p.ready ? '✅ READY' : '⏳ WAITING';
      const status = this.add.text(120, yOffset - 8, readyStatus, {
        fontFamily: 'monospace',
        fontSize: '15px',
        color: p.ready ? '#2ecc71' : '#f39c12',
      });

      this.playerListContainer.add([bg, dot, name, status]);
    });
  }
}
