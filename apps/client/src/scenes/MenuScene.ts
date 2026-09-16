import Phaser from 'phaser';
import { GameSocket } from '../net/GameSocket';

export class MenuScene extends Phaser.Scene {
  private displayName = 'Miner';
  private apiBase = (import.meta.env.VITE_API_BASE as string | undefined) || 'http://localhost:8787';

  constructor() {
    super('menu');
  }

  create(): void {
    const width = this.cameras.main.width;
    const height = this.cameras.main.height;

    // Load saved name
    try {
      const savedName = localStorage.getItem('minebombers_name');
      if (savedName) this.displayName = savedName;
    } catch { /* storage disabled */ }

    // Title & Logo
    this.add.text(width / 2, 70, 'MINE BOMBERS', {
      fontFamily: 'monospace',
      fontSize: '48px',
      fontStyle: 'bold',
      color: '#f39c12',
    }).setOrigin(0.5);

    this.add.text(width / 2, 120, 'RETRO MULTIPLAYER MINING & DEMOLITION', {
      fontFamily: 'monospace',
      fontSize: '16px',
      color: '#bdc3c7',
    }).setOrigin(0.5);

    // Miner preview animation
    const previewMiner = this.add.sprite(width / 2, 175, 'miner', 0).setScale(3);
    previewMiner.play('miner_down');

    // Player Name display & edit button
    const nameText = this.add.text(width / 2, 240, `Name: ${this.displayName} [Edit]`, {
      fontFamily: 'monospace',
      fontSize: '18px',
      color: '#ecf0f1',
      backgroundColor: '#2c3e50',
      padding: { x: 12, y: 6 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    nameText.on('pointerdown', () => {
      const input = prompt('Enter miner name (max 16 chars):', this.displayName);
      if (input) {
        this.displayName = input.trim().slice(0, 16) || 'Miner';
        nameText.setText(`Name: ${this.displayName} [Edit]`);
        try { localStorage.setItem('minebombers_name', this.displayName); } catch { /* ignore */ }
      }
    });

    // Buttons
    let currentY = 300;

    // 1. Play Solo / Practice (Instant Play vs Bots)
    this.createButton(width / 2, currentY, '⚔️ PLAY SOLO PRACTICE (VS BOTS)', 0x27ae60, () => {
      this.scene.start('shop', {
        mode: 'solo',
        displayName: this.displayName,
        cash: 500,
        inventory: {
          selectedSlot: 0,
          items: { small_charge: 2 },
          upgrades: { pickaxe_1: 1 },
        },
        onSoloShopComplete: (cash: number, inventory: { selectedSlot: number; items: Record<string, number>; upgrades: Record<string, number> }) => {
          this.scene.start('game', {
            mode: 'solo',
            displayName: this.displayName,
            soloRoundIndex: 1,
            soloCash: cash,
            soloInventory: inventory,
          });
        },
      });
    });

    currentY += 60;

    // 2. Create Multiplayer Room
    const createBtn = this.createButton(width / 2, currentY, '🌐 CREATE MULTIPLAYER ROOM', 0x2980b9, async () => {
      createBtn.setText('Creating room...');
      try {
        const res = await fetch(`${this.apiBase}/api/rooms`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
        });
        if (!res.ok) throw new Error('Failed to create room');
        const data = (await res.json()) as { roomCode: string };
        this.joinMultiplayerRoom(data.roomCode, true);
      } catch (err) {
        alert(`Could not connect to server at ${this.apiBase}. Try Play Solo Practice mode or start the backend!`);
        createBtn.setText('🌐 CREATE MULTIPLAYER ROOM');
      }
    });

    currentY += 60;

    // 3. Join Multiplayer Room
    this.createButton(width / 2, currentY, '🔑 JOIN ROOM BY CODE', 0x8e44ad, () => {
      const code = prompt('Enter 6-letter Room Code:');
      if (code) {
        this.joinMultiplayerRoom(code.trim().toUpperCase(), false);
      }
    });

    currentY += 60;

    // Server Endpoint config button
    const serverConfig = this.add.text(width / 2, currentY, `Server: ${this.apiBase} [Change]`, {
      fontFamily: 'monospace',
      fontSize: '13px',
      color: '#7f8c8d',
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    serverConfig.on('pointerdown', () => {
      const newUrl = prompt('Enter Cloudflare Worker Server Base URL:', this.apiBase);
      if (newUrl) {
        this.apiBase = newUrl.trim();
        serverConfig.setText(`Server: ${this.apiBase} [Change]`);
      }
    });

    // Instructions footer
    this.add.text(width / 2, height - 40, 'Controls: WASD/Arrows: Move & Dig | SPACE: Bomb | E: Med Kit | 1..4: Weapons | F3: Debug', {
      fontFamily: 'monospace',
      fontSize: '13px',
      color: '#95a5a6',
    }).setOrigin(0.5);
  }

  private createButton(x: number, y: number, label: string, color: number, onClick: () => void): Phaser.GameObjects.Text {
    const btn = this.add.text(x, y, label, {
      fontFamily: 'monospace',
      fontSize: '18px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: `#${color.toString(16).padStart(6, '0')}`,
      padding: { x: 24, y: 12 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    btn.on('pointerover', () => btn.setAlpha(0.85));
    btn.on('pointerout', () => btn.setAlpha(1.0));
    btn.on('pointerdown', onClick);

    return btn;
  }

  private joinMultiplayerRoom(roomCode: string, isHost: boolean): void {
    const socket = new GameSocket({
      apiBase: this.apiBase,
      roomCode,
      name: this.displayName,
      clientBuild: '0.1.0',
      onMessage: () => {},
    });

    this.scene.start('lobby', {
      roomCode,
      displayName: this.displayName,
      apiBase: this.apiBase,
      socket,
      isHost,
    });
  }
}
