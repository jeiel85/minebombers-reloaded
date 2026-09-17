import Phaser from 'phaser';
import { GameSocket } from '../net/GameSocket';
import { CLASSIC_MAP_NAMES } from '@minebombers/shared';
import { RetroAudio } from '../audio/RetroAudio';

export class MenuScene extends Phaser.Scene {
  private displayName = 'Miner';
  private apiBase = (import.meta.env.VITE_API_BASE as string | undefined) || 'http://localhost:8787';
  private mapOptions = ['RANDOM (Procedural)', ...CLASSIC_MAP_NAMES];
  private selectedMapIndex = 0;
  private mapText!: Phaser.GameObjects.Text;
  private mapSubtitle!: Phaser.GameObjects.Text;

  private botCount = 3;
  private botDifficulty: 'easy' | 'normal' | 'hardcore' = 'normal';
  private botCountText!: Phaser.GameObjects.Text;
  private botDiffText!: Phaser.GameObjects.Text;

  constructor() {
    super('menu');
  }

  private getSelectedMapPreset(): string | undefined {
    if (this.selectedMapIndex === 0) return undefined;
    return this.mapOptions[this.selectedMapIndex];
  }

  create(): void {
    const width = this.cameras.main.width;
    const height = this.cameras.main.height;

    // First interaction starts retro BGM & Sound Blaster audio
    this.input.on('pointerdown', () => {
      RetroAudio.playBGM('huippe');
    });

    // Load saved settings
    try {
      const savedName = localStorage.getItem('minebombers_name');
      if (savedName) this.displayName = savedName;
      const savedBots = localStorage.getItem('minebombers_bot_count');
      if (savedBots) this.botCount = Math.max(1, Math.min(7, parseInt(savedBots, 10) || 3));
      const savedDiff = localStorage.getItem('minebombers_bot_diff');
      if (savedDiff && ['easy', 'normal', 'hardcore'].includes(savedDiff)) {
        this.botDifficulty = savedDiff as 'easy' | 'normal' | 'hardcore';
      }
    } catch { /* storage disabled */ }

    // Audio Controls (Top Right)
    const bgmToggle = this.add.text(width - 150, 24, `🎵 BGM: ${RetroAudio.isMusicEnabled() ? 'ON' : 'OFF'}`, {
      fontFamily: 'monospace',
      fontSize: '13px',
      color: RetroAudio.isMusicEnabled() ? '#2ecc71' : '#7f8c8d',
      backgroundColor: '#1b2631',
      padding: { x: 8, y: 4 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    bgmToggle.on('pointerdown', () => {
      RetroAudio.playClick();
      const next = !RetroAudio.isMusicEnabled();
      RetroAudio.setMusicEnabled(next);
      bgmToggle.setText(`🎵 BGM: ${next ? 'ON' : 'OFF'}`);
      bgmToggle.setColor(next ? '#2ecc71' : '#7f8c8d');
      if (next) RetroAudio.playBGM('huippe');
    });

    const sfxToggle = this.add.text(width - 55, 24, `🔊 SFX: ${RetroAudio.isEnabled() ? 'ON' : 'OFF'}`, {
      fontFamily: 'monospace',
      fontSize: '13px',
      color: RetroAudio.isEnabled() ? '#2ecc71' : '#7f8c8d',
      backgroundColor: '#1b2631',
      padding: { x: 8, y: 4 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    sfxToggle.on('pointerdown', () => {
      RetroAudio.playClick();
      const next = !RetroAudio.isEnabled();
      RetroAudio.setEnabled(next);
      sfxToggle.setText(`🔊 SFX: ${next ? 'ON' : 'OFF'}`);
      sfxToggle.setColor(next ? '#2ecc71' : '#7f8c8d');
    });

    // Title & Logo
    this.add.text(width / 2, 50, 'MINE BOMBERS', {
      fontFamily: 'monospace',
      fontSize: '42px',
      fontStyle: 'bold',
      color: '#f39c12',
    }).setOrigin(0.5);

    this.add.text(width / 2, 86, 'RETRO MULTIPLAYER MINING & DEMOLITION', {
      fontFamily: 'monospace',
      fontSize: '14px',
      color: '#bdc3c7',
    }).setOrigin(0.5);

    // Miner preview animation
    const previewMiner = this.add.sprite(width / 2, 132, 'miner', 0).setScale(2.2);
    previewMiner.play('miner_down');

    // Player Name display & edit button
    const nameText = this.add.text(width / 2, 180, `Name: ${this.displayName} [Edit]`, {
      fontFamily: 'monospace',
      fontSize: '15px',
      color: '#ecf0f1',
      backgroundColor: '#2c3e50',
      padding: { x: 12, y: 5 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    nameText.on('pointerdown', () => {
      RetroAudio.playClick();
      const input = prompt('Enter miner name (max 16 chars):', this.displayName);
      if (input) {
        this.displayName = input.trim().slice(0, 16) || 'Miner';
        nameText.setText(`Name: ${this.displayName} [Edit]`);
        try { localStorage.setItem('minebombers_name', this.displayName); } catch { /* ignore */ }
      }
    });

    // Map Selector UI (Classic 46 Maps vs Procedural)
    const mapY = 226;
    const prevMapBtn = this.add.text(width / 2 - 210, mapY, '◀', {
      fontFamily: 'monospace',
      fontSize: '20px',
      fontStyle: 'bold',
      color: '#f1c40f',
      backgroundColor: '#1f242d',
      padding: { x: 10, y: 4 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    this.mapText = this.add.text(width / 2, mapY, this.getMapLabel(), {
      fontFamily: 'monospace',
      fontSize: '16px',
      fontStyle: 'bold',
      color: '#00ffcc',
      backgroundColor: '#1b2631',
      padding: { x: 16, y: 5 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    const nextMapBtn = this.add.text(width / 2 + 210, mapY, '▶', {
      fontFamily: 'monospace',
      fontSize: '20px',
      fontStyle: 'bold',
      color: '#f1c40f',
      backgroundColor: '#1f242d',
      padding: { x: 10, y: 4 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    this.mapSubtitle = this.add.text(width / 2, mapY + 25, this.getMapSubLabel(), {
      fontFamily: 'monospace',
      fontSize: '12px',
      color: '#95a5a6',
    }).setOrigin(0.5);

    const updateMapUI = () => {
      this.mapText.setText(this.getMapLabel());
      this.mapSubtitle.setText(this.getMapSubLabel());
    };

    prevMapBtn.on('pointerdown', () => {
      RetroAudio.playClick();
      this.selectedMapIndex = (this.selectedMapIndex - 1 + this.mapOptions.length) % this.mapOptions.length;
      updateMapUI();
    });

    nextMapBtn.on('pointerdown', () => {
      RetroAudio.playClick();
      this.selectedMapIndex = (this.selectedMapIndex + 1) % this.mapOptions.length;
      updateMapUI();
    });

    this.mapText.on('pointerdown', () => {
      RetroAudio.playClick();
      const input = prompt(`Enter Map Name or Index (1..${CLASSIC_MAP_NAMES.length}):`, this.mapOptions[this.selectedMapIndex]);
      if (input) {
        const query = input.trim().toUpperCase();
        const num = parseInt(query, 10);
        if (!isNaN(num) && num >= 1 && num <= CLASSIC_MAP_NAMES.length) {
          this.selectedMapIndex = num;
          updateMapUI();
        } else {
          const idx = this.mapOptions.findIndex((m) => m.toUpperCase().includes(query));
          if (idx >= 0) {
            this.selectedMapIndex = idx;
            updateMapUI();
          }
        }
      }
    });

    // --- Bot Configuration Controls (Bot Count & Difficulty) ---
    const botY = 282;

    // Bot Count Selector: ◀ 🤖 BOTS: 3 (4 Miners) ▶
    const prevBotBtn = this.add.text(width / 2 - 220, botY, '◀', {
      fontFamily: 'monospace',
      fontSize: '18px',
      fontStyle: 'bold',
      color: '#3498db',
      backgroundColor: '#1f242d',
      padding: { x: 8, y: 3 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    this.botCountText = this.add.text(width / 2 - 120, botY, this.getBotCountLabel(), {
      fontFamily: 'monospace',
      fontSize: '14px',
      fontStyle: 'bold',
      color: '#ecf0f1',
      backgroundColor: '#243342',
      padding: { x: 10, y: 5 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    const nextBotBtn = this.add.text(width / 2 - 20, botY, '▶', {
      fontFamily: 'monospace',
      fontSize: '18px',
      fontStyle: 'bold',
      color: '#3498db',
      backgroundColor: '#1f242d',
      padding: { x: 8, y: 3 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    const updateBotCount = (delta: number) => {
      RetroAudio.playClick();
      this.botCount += delta;
      if (this.botCount > 7) this.botCount = 1;
      if (this.botCount < 1) this.botCount = 7;
      this.botCountText.setText(this.getBotCountLabel());
      try { localStorage.setItem('minebombers_bot_count', String(this.botCount)); } catch { /* ignore */ }
    };

    prevBotBtn.on('pointerdown', () => updateBotCount(-1));
    nextBotBtn.on('pointerdown', () => updateBotCount(1));
    this.botCountText.on('pointerdown', () => updateBotCount(1));

    // Bot Difficulty Selector: ◀ ⚡ AI: NORMAL ▶
    const prevDiffBtn = this.add.text(width / 2 + 20, botY, '◀', {
      fontFamily: 'monospace',
      fontSize: '18px',
      fontStyle: 'bold',
      color: '#e67e22',
      backgroundColor: '#1f242d',
      padding: { x: 8, y: 3 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    this.botDiffText = this.add.text(width / 2 + 120, botY, this.getBotDiffLabel(), {
      fontFamily: 'monospace',
      fontSize: '14px',
      fontStyle: 'bold',
      color: this.getBotDiffColor(),
      backgroundColor: '#243342',
      padding: { x: 12, y: 5 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    const nextDiffBtn = this.add.text(width / 2 + 220, botY, '▶', {
      fontFamily: 'monospace',
      fontSize: '18px',
      fontStyle: 'bold',
      color: '#e67e22',
      backgroundColor: '#1f242d',
      padding: { x: 8, y: 3 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    const difficulties: Array<'easy' | 'normal' | 'hardcore'> = ['easy', 'normal', 'hardcore'];
    const updateBotDifficulty = (delta: number) => {
      RetroAudio.playClick();
      const currIdx = difficulties.indexOf(this.botDifficulty);
      const nextIdx = (currIdx + delta + difficulties.length) % difficulties.length;
      this.botDifficulty = difficulties[nextIdx]!;
      this.botDiffText.setText(this.getBotDiffLabel());
      this.botDiffText.setColor(this.getBotDiffColor());
      try { localStorage.setItem('minebombers_bot_diff', this.botDifficulty); } catch { /* ignore */ }
    };

    prevDiffBtn.on('pointerdown', () => updateBotDifficulty(-1));
    nextDiffBtn.on('pointerdown', () => updateBotDifficulty(1));
    this.botDiffText.on('pointerdown', () => updateBotDifficulty(1));

    // Buttons
    let currentY = 338;

    // 1. Play Solo Battle (Instant Play vs Configured Bots)
    this.createButton(width / 2, currentY, '⚔️ PLAY SOLO BATTLE (VS BOTS)', 0x27ae60, () => {
      RetroAudio.playClick();
      const selectedMap = this.getSelectedMapPreset();
      this.scene.start('shop', {
        mode: 'solo',
        displayName: this.displayName,
        selectedMap,
        botCount: this.botCount,
        botDifficulty: this.botDifficulty,
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
            selectedMap,
            botCount: this.botCount,
            botDifficulty: this.botDifficulty,
            soloRoundIndex: 1,
            soloCash: cash,
            soloInventory: inventory,
          });
        },
      });
    });

    currentY += 58;

    // 2. Play 100% Original DOS PC Version
    this.createButton(width / 2, currentY, '🕹️ PLAY ORIGINAL PC DOS VERSION (1995)', 0xd35400, () => {
      RetroAudio.playClick();
      window.location.href = './dos.html';
    });

    currentY += 60;

    // 2. Create Multiplayer Room
    const createBtn = this.createButton(width / 2, currentY, '🌐 CREATE MULTIPLAYER ROOM', 0x2980b9, async () => {
      RetroAudio.playClick();
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
      RetroAudio.playClick();
      const code = prompt('Enter 6-letter Room Code:');
      if (code) {
        this.joinMultiplayerRoom(code.trim().toUpperCase(), false);
      }
    });

    currentY += 58;

    // Server Endpoint config button
    const serverConfig = this.add.text(width / 2, currentY, `Server: ${this.apiBase} [Change]`, {
      fontFamily: 'monospace',
      fontSize: '13px',
      color: '#7f8c8d',
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    serverConfig.on('pointerdown', () => {
      RetroAudio.playClick();
      const newUrl = prompt('Enter Cloudflare Worker Server Base URL:', this.apiBase);
      if (newUrl) {
        this.apiBase = newUrl.trim();
        serverConfig.setText(`Server: ${this.apiBase} [Change]`);
      }
    });

    // Instructions footer
    this.add.text(width / 2, height - 35, 'Controls: WASD/Arrows: Move & Dig | SPACE: Bomb | E: Med Kit | 1..4: Weapons | F3: Debug', {
      fontFamily: 'monospace',
      fontSize: '13px',
      color: '#95a5a6',
    }).setOrigin(0.5);
  }

  private getMapLabel(): string {
    if (this.selectedMapIndex === 0) {
      return 'MAP: 🎲 RANDOM MINE';
    }
    const name = this.mapOptions[this.selectedMapIndex]!;
    return `MAP: 📜 ${name} (${this.selectedMapIndex}/${CLASSIC_MAP_NAMES.length})`;
  }

  private getMapSubLabel(): string {
    if (this.selectedMapIndex === 0) {
      return 'Procedural Cavern Generator (Dynamic Spawns & Ores)';
    }
    return 'Official 1995 DOS Classic Arena (Skaven / Tuomas Ihme)';
  }

  private getBotCountLabel(): string {
    const total = 1 + this.botCount;
    return `🤖 BOTS: ${this.botCount} (${total} Miners)`;
  }

  private getBotDiffLabel(): string {
    return `⚡ AI: ${this.botDifficulty.toUpperCase()}`;
  }

  private getBotDiffColor(): string {
    switch (this.botDifficulty) {
      case 'easy':
        return '#2ecc71';
      case 'normal':
        return '#f1c40f';
      case 'hardcore':
        return '#e74c3c';
    }
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
