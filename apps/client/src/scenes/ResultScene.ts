import Phaser from 'phaser';
import type { GameSocket } from '../net/GameSocket';
import type { SimMatchStandingsEntry, SimRoundStandingsEntry, ServerMessage } from '@minebombers/shared';
import { AssetRegistry } from '../game/AssetRegistry';

export interface ResultSceneData {
  mode?: 'multiplayer' | 'solo';
  roomCode?: string;
  displayName?: string;
  socket?: GameSocket;
  myPlayerId?: string;
  roundIndex: number;
  totalRounds: number;
  roundStandings?: SimRoundStandingsEntry[];
  matchStandings?: SimMatchStandingsEntry[];
  isMatchEnd: boolean;
  onSoloNextRound?: () => void;
}

export class ResultScene extends Phaser.Scene {
  private dataPayload!: ResultSceneData;
  private timerText!: Phaser.GameObjects.Text;
  private secondsLeft = 8;
  private countdownTimer: ReturnType<typeof setInterval> | null = null;

  constructor() {
    super('result');
  }

  init(data: ResultSceneData): void {
    this.dataPayload = data;
    this.secondsLeft = 8;
  }

  create(): void {
    const width = this.cameras.main.width;
    const height = this.cameras.main.height;

    const titleText = this.dataPayload.isMatchEnd ? '🏆 FINAL MATCH RESULTS 🏆' : `ROUND ${this.dataPayload.roundIndex} RESULTS`;
    this.add.text(width / 2, 50, titleText, {
      fontFamily: 'monospace',
      fontSize: '32px',
      fontStyle: 'bold',
      color: '#f39c12',
    }).setOrigin(0.5);

    if (this.dataPayload.isMatchEnd) {
      this.renderMatchStandings(width, height);
    } else {
      this.renderRoundStandings(width, height);
    }

    // Socket message listening if multiplayer
    if (this.dataPayload.socket) {
      (this.dataPayload.socket as unknown as { options: { onMessage: (msg: ServerMessage) => void } }).options.onMessage = (msg: ServerMessage) => {
        if (msg.t === 's.shop') {
          this.scene.start('shop', {
            ...this.dataPayload,
            shopData: msg,
          });
        }
      };
    }
  }

  private renderRoundStandings(width: number, height: number): void {
    const standings = this.dataPayload.roundStandings ?? [];

    this.add.text(width / 2, 100, 'STANDINGS BY CASH THIS MATCH', {
      fontFamily: 'monospace',
      fontSize: '16px',
      color: '#bdc3c7',
    }).setOrigin(0.5);

    // Table Header
    const headerY = 140;
    this.add.rectangle(width / 2, headerY, width - 160, 36, 0x2c3e50, 0.9);
    this.add.text(width / 2 - 300, headerY - 8, 'RANK & MINER', { fontFamily: 'monospace', fontSize: '15px', fontStyle: 'bold', color: '#f1c40f' });
    this.add.text(width / 2, headerY - 8, 'STATUS | KILLS', { fontFamily: 'monospace', fontSize: '15px', fontStyle: 'bold', color: '#f1c40f' });
    this.add.text(width / 2 + 220, headerY - 8, 'CASH', { fontFamily: 'monospace', fontSize: '15px', fontStyle: 'bold', color: '#f1c40f' });

    // Table Rows
    standings.forEach((entry, index) => {
      const rowY = headerY + 45 + index * 42;
      const isMe = entry.playerId === this.dataPayload.myPlayerId;
      const color = AssetRegistry.getPlayerColor(index);

      const bg = this.add.rectangle(width / 2, rowY, width - 160, 38, isMe ? 0x1a3a4b : 0x1f242d, 0.9);
      this.add.circle(width / 2 - 350, rowY, 6, color);

      const rankBadge = index === 0 ? '🥇' : index === 1 ? '🥈' : index === 2 ? '🥉' : `#${index + 1}`;
      this.add.text(width / 2 - 330, rowY - 8, `${rankBadge} ${entry.playerId}`, {
        fontFamily: 'monospace',
        fontSize: '15px',
        color: isMe ? '#00ffcc' : '#ffffff',
      });

      const status = entry.alive ? '🟢 Survived' : '💀 Eliminated';
      this.add.text(width / 2, rowY - 8, `${status} | Kills: ${entry.kills}`, {
        fontFamily: 'monospace',
        fontSize: '15px',
        color: '#ecf0f1',
      });

      this.add.text(width / 2 + 220, rowY - 8, `$${entry.cash}`, {
        fontFamily: 'monospace',
        fontSize: '16px',
        fontStyle: 'bold',
        color: '#2ecc71',
      });
    });

    // Countdown
    this.timerText = this.add.text(width / 2, height - 70, `Next Shop in ${this.secondsLeft}s...`, {
      fontFamily: 'monospace',
      fontSize: '20px',
      color: '#e74c3c',
    }).setOrigin(0.5);

    this.countdownTimer = setInterval(() => {
      this.secondsLeft--;
      if (this.timerText) {
        this.timerText.setText(`Next Shop in ${this.secondsLeft}s...`);
      }
      if (this.secondsLeft <= 0) {
        if (this.countdownTimer) clearInterval(this.countdownTimer);
        if (this.dataPayload.mode === 'solo') {
          this.dataPayload.onSoloNextRound?.();
        }
      }
    }, 1000);
  }

  private renderMatchStandings(width: number, height: number): void {
    const standings = this.dataPayload.matchStandings ?? [];

    // Winner announcement
    const winner = standings[0];
    if (winner) {
      this.add.text(width / 2, 110, `👑 WINNER: ${winner.playerId} 👑`, {
        fontFamily: 'monospace',
        fontSize: '28px',
        fontStyle: 'bold',
        color: '#f1c40f',
      }).setOrigin(0.5);
    }

    // Standings table
    standings.forEach((entry, index) => {
      const rowY = 180 + index * 50;
      const isMe = entry.playerId === this.dataPayload.myPlayerId;
      const bg = this.add.rectangle(width / 2, rowY, 600, 42, isMe ? 0x1a3a4b : 0x1f242d, 0.95);
      bg.setStrokeStyle(1, index === 0 ? 0xf1c40f : 0x34495e);

      const medal = index === 0 ? '🥇 1st' : index === 1 ? '🥈 2nd' : index === 2 ? '🥉 3rd' : `${index + 1}th`;
      this.add.text(width / 2 - 260, rowY - 9, medal, { fontFamily: 'monospace', fontSize: '18px', fontStyle: 'bold', color: '#f1c40f' });
      this.add.text(width / 2 - 170, rowY - 9, entry.playerId, { fontFamily: 'monospace', fontSize: '17px', color: '#ffffff' });
      this.add.text(width / 2 + 30, rowY - 9, `Wins: ${entry.roundWins} | Kills: ${entry.kills}`, { fontFamily: 'monospace', fontSize: '14px', color: '#bdc3c7' });
      this.add.text(width / 2 + 200, rowY - 9, `$${entry.cash}`, { fontFamily: 'monospace', fontSize: '18px', fontStyle: 'bold', color: '#2ecc71' });
    });

    // Return to Menu Button
    const menuBtn = this.add.text(width / 2, height - 60, 'RETURN TO MENU', {
      fontFamily: 'monospace',
      fontSize: '20px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: '#27ae60',
      padding: { x: 32, y: 12 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    menuBtn.on('pointerdown', () => {
      this.dataPayload.socket?.close();
      this.scene.start('menu');
    });
  }

  shutdown(): void {
    if (this.countdownTimer) {
      clearInterval(this.countdownTimer);
      this.countdownTimer = null;
    }
  }
}
