import Phaser from 'phaser';
import type { GameSocket } from '../net/GameSocket';
import {
  MAP_HEIGHT,
  MAP_WIDTH,
  TILE_SIZE_PX,
  WORLD_UNITS_PER_TILE,
  WorldSimulation,
  calculateMatchStandings,
  createDefaultInventory,
  generateClassicMine,
  type EntityView,
  type GameEvent,
  type InventoryState,
  type ServerMessage,
  type SimTileState,
} from '@minebombers/shared';
import { AssetRegistry } from '../game/AssetRegistry';
import { InputController } from '../game/InputController';
import { ClientPrediction } from '../game/Prediction';
import { RemoteEntityInterpolation } from '../game/Interpolation';
import { DebugOverlay } from '../ui/DebugOverlay';
import { RetroAudio } from '../audio/RetroAudio';

export const HOTBAR_ITEMS = [
  { id: 'small_charge', name: 'Small Bomb', icon: 0 },
  { id: 'dynamite', name: 'Dynamite', icon: 1 },
  { id: 'heavy_charge', name: 'Heavy Bomb', icon: 2 },
  { id: 'remote_bomb', name: 'Remote Bomb', icon: 3 },
  { id: 'proximity_mine', name: 'Landmine', icon: 4 },
  { id: 'rocket', name: 'Mini-Rocket', icon: 5 },
  { id: 'flamethrower', name: 'Flamethrower', icon: 6 },
  { id: 'nuke', name: 'Nuke', icon: 7 },
];

export interface GameSceneData {
  mode: 'multiplayer' | 'solo';
  roomCode?: string;
  displayName: string;
  socket?: GameSocket;
  myPlayerId?: string;
  startData?: Extract<ServerMessage, { t: 's.start' }>;
  soloRoundIndex?: number;
  soloCash?: number;
  soloInventory?: ReturnType<typeof createDefaultInventory>;
}

export class GameScene extends Phaser.Scene {
  private dataPayload!: GameSceneData;
  private socket?: GameSocket;
  private myPlayerId = '';

  // Rendering Layers
  private floorLayer!: Phaser.GameObjects.TileSprite;
  private tileSprites = new Map<number, Phaser.GameObjects.Sprite>();
  private playerSprites = new Map<string, Phaser.GameObjects.Sprite>();
  private playerLabels = new Map<string, Phaser.GameObjects.Text>();
  private playerPrevPositions = new Map<string, { x: number; y: number }>();
  private entitySprites = new Map<string, Phaser.GameObjects.Sprite>();

  // HUD
  private hudBg!: Phaser.GameObjects.Rectangle;
  private hpText!: Phaser.GameObjects.Text;
  private hpBar!: Phaser.GameObjects.Rectangle;
  private cashText!: Phaser.GameObjects.Text;
  private ammoText!: Phaser.GameObjects.Text;
  private timerText!: Phaser.GameObjects.Text;
  private aliveText!: Phaser.GameObjects.Text;

  // Bottom Hotbar HUD
  private hotbarBorders: Phaser.GameObjects.Rectangle[] = [];
  private hotbarSlotBgs: Phaser.GameObjects.Rectangle[] = [];
  private hotbarCounts: Phaser.GameObjects.Text[] = [];

  // Controllers & Net
  private inputController!: InputController;
  private prediction = new ClientPrediction();
  private remoteInterpolations = new Map<string, RemoteEntityInterpolation>();
  private debugOverlay!: DebugOverlay;

  // Local WorldSimulation for Solo Practice Mode
  private soloSim: WorldSimulation | null = null;
  private soloTimer: ReturnType<typeof setInterval> | null = null;
  private botAiTimer: ReturnType<typeof setInterval> | null = null;
  private soloRoundIndex = 1;
  private totalRounds = 5;
  private botStates = new Map<string, { dx: -1 | 0 | 1; dy: -1 | 0 | 1; holdMs: number; retreatMs: number }>();
  private isPaused = false;
  private pauseContainer: Phaser.GameObjects.Container | null = null;

  // Tiles State
  private currentTiles: SimTileState[] = [];
  private lastServerSeq = 0;
  private roundEndsAt = 0;

  constructor() {
    super('game');
  }

  init(data: GameSceneData): void {
    this.dataPayload = data;
    this.socket = data.socket;
    this.myPlayerId = data.myPlayerId ?? 'player_1';
    this.soloRoundIndex = data.soloRoundIndex ?? 1;
    this.isPaused = false;
    this.botStates.clear();
  }

  create(): void {
    const arenaWidth = MAP_WIDTH * TILE_SIZE_PX; // 992
    const arenaHeight = MAP_HEIGHT * TILE_SIZE_PX; // 736

    // 1. Floor Background Layer
    this.floorLayer = this.add.tileSprite(0, 0, arenaWidth, arenaHeight, 'tiles_world', 0).setOrigin(0, 0);

    // Initialize Input & Debug
    this.inputController = new InputController(this);
    this.debugOverlay = new DebugOverlay(this);

    // Create HUD & Hotbar
    this.createHUD(arenaWidth);
    this.createHotbar(arenaWidth, arenaHeight);

    if (this.dataPayload.mode === 'solo') {
      this.initSoloGame();
    } else {
      this.initMultiplayerGame();
    }
  }

  private createHUD(arenaWidth: number): void {
    const hudHeight = 44;
    this.hudBg = this.add.rectangle(0, 0, arenaWidth, hudHeight, 0x111111, 0.85).setOrigin(0, 0);
    this.hudBg.setDepth(500);

    // HP Bar
    this.add.text(16, 12, 'HP:', { fontFamily: 'monospace', fontSize: '15px', color: '#ffffff' }).setDepth(501);
    this.hpBar = this.add.rectangle(50, 14, 100, 16, 0x2ecc71).setOrigin(0, 0).setDepth(501);
    this.hpText = this.add.text(160, 12, '100/100', { fontFamily: 'monospace', fontSize: '14px', color: '#ffffff' }).setDepth(501);

    // Cash
    this.cashText = this.add.text(250, 12, 'CASH: $500', {
      fontFamily: 'monospace',
      fontSize: '16px',
      fontStyle: 'bold',
      color: '#2ecc71',
    }).setDepth(501);

    // Ammo / Equipped
    this.ammoText = this.add.text(390, 12, '💣 Small Bomb: 2', {
      fontFamily: 'monospace',
      fontSize: '15px',
      color: '#f1c40f',
    }).setDepth(501);

    // Timer
    this.timerText = this.add.text(640, 12, 'TIME: 05:00', {
      fontFamily: 'monospace',
      fontSize: '16px',
      fontStyle: 'bold',
      color: '#e74c3c',
    }).setDepth(501);

    // Alive
    this.aliveText = this.add.text(780, 12, 'MINERS: 4/4', {
      fontFamily: 'monospace',
      fontSize: '15px',
      color: '#3498db',
    }).setDepth(501);

    // Pause / Menu button
    const menuBtn = this.add.text(arenaWidth - 12, 10, '⚙️ MENU', {
      fontFamily: 'monospace',
      fontSize: '14px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: '#34495e',
      padding: { x: 8, y: 4 },
    }).setOrigin(1, 0).setDepth(501).setInteractive({ useHandCursor: true });

    menuBtn.on('pointerdown', () => this.togglePauseMenu());
  }

  private createHotbar(arenaWidth: number, arenaHeight: number): void {
    const slotCount = HOTBAR_ITEMS.length; // 8
    const slotSize = 42;
    const gap = 6;
    const totalWidth = slotCount * slotSize + (slotCount - 1) * gap;
    const startX = (arenaWidth - totalWidth) / 2 + slotSize / 2;
    const posY = arenaHeight - 26;

    // Panel background
    const bgPanel = this.add.rectangle(arenaWidth / 2, posY, totalWidth + 20, 48, 0x0e1117, 0.9).setDepth(500);
    bgPanel.setStrokeStyle(1, 0x2c3e50);

    for (let i = 0; i < slotCount; i++) {
      const item = HOTBAR_ITEMS[i]!;
      const sx = startX + i * (slotSize + gap);

      const slotBg = this.add.rectangle(sx, posY, slotSize, slotSize, 0x1a202c, 0.95).setDepth(501);
      const border = this.add.rectangle(sx, posY, slotSize, slotSize).setStrokeStyle(1, 0x34495e).setDepth(502);
      this.add.sprite(sx, posY, 'ui_icons', item.icon).setScale(0.9).setDepth(503);

      this.add.text(sx - 17, posY - 18, `${i + 1}`, {
        fontFamily: 'monospace',
        fontSize: '11px',
        fontStyle: 'bold',
        color: '#f39c12',
      }).setDepth(504);

      const count = this.add.text(sx + 18, posY + 6, '0', {
        fontFamily: 'monospace',
        fontSize: '11px',
        fontStyle: 'bold',
        color: '#7f8c8d',
      }).setOrigin(1, 0).setDepth(504);

      slotBg.setInteractive({ useHandCursor: true });
      slotBg.on('pointerdown', () => {
        this.inputController.setSlot(i);
        RetroAudio.playClick();
      });

      this.hotbarSlotBgs.push(slotBg);
      this.hotbarBorders.push(border);
      this.hotbarCounts.push(count);
    }
  }

  private updateHotbar(inventory: InventoryState, selectedSlot: number): void {
    for (let i = 0; i < HOTBAR_ITEMS.length; i++) {
      const item = HOTBAR_ITEMS[i]!;
      const count = inventory.items[item.id] ?? 0;
      const isSelected = i === selectedSlot;

      const border = this.hotbarBorders[i];
      const bg = this.hotbarSlotBgs[i];
      const countTxt = this.hotbarCounts[i];

      if (border && bg && countTxt) {
        if (isSelected) {
          border.setStrokeStyle(2, 0xf1c40f);
          bg.fillColor = 0x2c3e50;
        } else {
          border.setStrokeStyle(1, 0x34495e);
          bg.fillColor = 0x1a202c;
        }

        countTxt.setText(`${count}`);
        countTxt.setColor(count > 0 ? '#2ecc71' : '#7f8c8d');
      }
    }

    const currentItem = HOTBAR_ITEMS[selectedSlot];
    const currentCount = currentItem ? (inventory.items[currentItem.id] ?? 0) : 0;
    this.ammoText.setText(`[${selectedSlot + 1}] ${currentItem?.name ?? 'Bomb'}: ${currentCount}`);
  }

  private togglePauseMenu(): void {
    if (this.pauseContainer) {
      this.pauseContainer.destroy();
      this.pauseContainer = null;
      this.isPaused = false;
      return;
    }

    this.isPaused = true;
    const width = this.cameras.main.width;
    const height = this.cameras.main.height;

    const bg = this.add.rectangle(width / 2, height / 2, width, height, 0x000000, 0.75).setInteractive();
    const panel = this.add.rectangle(width / 2, height / 2, 360, 260, 0x1f242d, 0.95);
    panel.setStrokeStyle(2, 0xe67e22);

    const title = this.add.text(width / 2, height / 2 - 80, 'GAME PAUSED', {
      fontFamily: 'monospace',
      fontSize: '22px',
      fontStyle: 'bold',
      color: '#f39c12',
    }).setOrigin(0.5);

    const resumeBtn = this.add.text(width / 2, height / 2 - 25, '▶️ RESUME', {
      fontFamily: 'monospace',
      fontSize: '16px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: '#27ae60',
      padding: { x: 20, y: 8 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });
    resumeBtn.on('pointerdown', () => this.togglePauseMenu());

    const skipBtn = this.add.text(width / 2, height / 2 + 30, '⚡ SKIP ROUND (SURRENDER)', {
      fontFamily: 'monospace',
      fontSize: '15px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: '#d35400',
      padding: { x: 16, y: 8 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });
    skipBtn.on('pointerdown', () => {
      this.togglePauseMenu();
      if (this.dataPayload.mode === 'solo' && this.soloSim) {
        const myP = this.soloSim.players.find((p) => p.id === this.myPlayerId);
        if (myP) {
          myP.hp = 0;
          myP.alive = false;
        }
        this.soloSim.phase = 'round_result';
        this.handleSoloRoundEnd();
      }
    });

    const quitBtn = this.add.text(width / 2, height / 2 + 85, '🚪 QUIT TO MAIN MENU', {
      fontFamily: 'monospace',
      fontSize: '15px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: '#c0392b',
      padding: { x: 16, y: 8 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });
    quitBtn.on('pointerdown', () => {
      this.togglePauseMenu();
      if (this.soloTimer) clearInterval(this.soloTimer);
      if (this.botAiTimer) clearInterval(this.botAiTimer);
      this.scene.start('menu');
    });

    this.pauseContainer = this.add.container(0, 0, [bg, panel, title, resumeBtn, skipBtn, quitBtn]).setDepth(1000);
  }

  // --- SOLO PRACTICE MODE ---
  private initSoloGame(): void {
    const seed = Math.floor(Math.random() * 1000000);
    const map = generateClassicMine(seed, 4);

    this.currentTiles = map.tiles.map((kind) => ({ kind, durability: kind === 'soil' ? 1000 : 0 }));
    this.renderInitialMap(map.tiles);

    const soloInventory = this.dataPayload.soloInventory ?? createDefaultInventory();
    const soloCash = this.dataPayload.soloCash ?? 500;

    const initialPlayers = [
      { id: this.myPlayerId, name: this.dataPayload.displayName, cash: soloCash, inventory: soloInventory },
      { id: 'bot_1', name: 'Garry (Bot)', cash: 500, inventory: createDefaultInventory() },
      { id: 'bot_2', name: 'Rusty (Bot)', cash: 500, inventory: createDefaultInventory() },
      { id: 'bot_3', name: 'Dynamo (Bot)', cash: 500, inventory: createDefaultInventory() },
    ];

    this.soloSim = new WorldSimulation(seed, map, initialPlayers, this.soloRoundIndex, 300_000);
    this.roundEndsAt = Date.now() + 300_000;

    // Init local prediction with human player spawn
    const mySimP = this.soloSim.players[0]!;
    this.prediction.init(mySimP.x, mySimP.y);

    // 20 Hz Simulation Step
    this.soloTimer = setInterval(() => {
      if (!this.soloSim || this.isPaused) return;
      const events = this.soloSim.step(50);
      this.handleGameEvents(events);

      // Check for round end
      if (this.soloSim.phase === 'round_result') {
        this.handleSoloRoundEnd();
      }
    }, 50);

    // Bot AI Decision Loop (~100ms)
    this.botAiTimer = setInterval(() => {
      this.runBotAI(100);
    }, 100);
  }

  private runBotAI(stepMs: number): void {
    if (!this.soloSim || this.soloSim.phase !== 'playing' || this.isPaused) return;

    const directions: Array<[-1 | 0 | 1, -1 | 0 | 1]> = [
      [0, -1], [0, 1], [-1, 0], [1, 0],
    ];

    for (let i = 1; i < this.soloSim.players.length; i++) {
      const bot = this.soloSim.players[i]!;
      if (!bot.alive) continue;

      let state = this.botStates.get(bot.id);
      if (!state) {
        state = { dx: 0, dy: 1, holdMs: 1500, retreatMs: 0 };
        this.botStates.set(bot.id, state);
      }

      const botTileX = Math.floor(bot.x / WORLD_UNITS_PER_TILE);
      const botTileY = Math.floor(bot.y / WORLD_UNITS_PER_TILE);

      // 1. Retreat from nearby bombs if any
      if (state.retreatMs > 0) {
        state.retreatMs -= stepMs;
      } else {
        const nearBomb = this.soloSim.explosives.find(
          (e) => Math.abs(e.tileX - botTileX) + Math.abs(e.tileY - botTileY) <= 2,
        );
        if (nearBomb) {
          const fdx: -1 | 0 | 1 = botTileX < nearBomb.tileX ? -1 : botTileX > nearBomb.tileX ? 1 : 0;
          const fdy: -1 | 0 | 1 = botTileY < nearBomb.tileY ? -1 : botTileY > nearBomb.tileY ? 1 : 0;
          state.dx = fdx !== 0 ? fdx : (Math.random() < 0.5 ? -1 : 1);
          state.dy = fdy !== 0 && fdx === 0 ? fdy : 0;
          state.retreatMs = 1800;
          state.holdMs = 1800;
        }
      }

      // 2. Opponent proximity check (Melee / Bomb)
      let primaryAction = false;
      let secondaryAction = false;
      let nearOpponent = false;

      for (const opp of this.soloSim.players) {
        if (opp.id === bot.id || !opp.alive) continue;
        const dist = Math.hypot(opp.x - bot.x, opp.y - bot.y);
        if (dist <= 1200) {
          nearOpponent = true;
          const ammoCount = bot.inventory.items['small_charge'] ?? 0;
          if (ammoCount > 0 && Math.random() < 0.25) {
            primaryAction = true;
            // Retreat after planting bomb
            state.dx = bot.x < opp.x ? -1 : 1;
            state.dy = 0;
            state.retreatMs = 2000;
            state.holdMs = 2000;
          } else {
            // Out of ammo or close quarters -> melee pickaxe swing!
            primaryAction = true;
            const diffX = opp.x - bot.x;
            const diffY = opp.y - bot.y;
            if (Math.abs(diffX) > Math.abs(diffY)) {
              state.dx = diffX > 0 ? 1 : -1;
              state.dy = 0;
            } else {
              state.dx = 0;
              state.dy = diffY > 0 ? 1 : -1;
            }
          }
          break;
        }
      }

      // 3. Digging and Path Navigation
      if (state.retreatMs <= 0 && !nearOpponent) {
        state.holdMs -= stepMs;
        if (state.holdMs <= 0) {
          // Check for nearest uncollected treasure or pickup within 8 tiles
          let target: { tileX: number; tileY: number } | null = null;
          let minD = 999;

          for (const t of this.soloSim.treasures) {
            if (t.collected) continue;
            const d = Math.abs(t.tileX - botTileX) + Math.abs(t.tileY - botTileY);
            if (d < minD && d <= 8) {
              minD = d;
              target = t;
            }
          }

          if (!target) {
            for (const p of this.soloSim.pickups) {
              if (p.collected) continue;
              const d = Math.abs(p.tileX - botTileX) + Math.abs(p.tileY - botTileY);
              if (d < minD && d <= 8) {
                minD = d;
                target = p;
              }
            }
          }

          if (target) {
            const diffX = target.tileX - botTileX;
            const diffY = target.tileY - botTileY;
            if (Math.abs(diffX) > Math.abs(diffY) && diffX !== 0) {
              state.dx = diffX > 0 ? 1 : -1;
              state.dy = 0;
            } else if (diffY !== 0) {
              state.dx = 0;
              state.dy = diffY > 0 ? 1 : -1;
            }
          } else {
            // Pick a random cardinal direction
            const dir = directions[Math.floor(Math.random() * directions.length)]!;
            state.dx = dir[0];
            state.dy = dir[1];
          }

          state.holdMs = 1500; // Hold for 1.5s to ensure soil digging completes
        }
      }

      // 4. Med Kit use if hurt
      if (bot.hp <= 50 && (bot.inventory.items['med_kit'] ?? 0) > 0) {
        secondaryAction = true;
      }

      this.soloSim.setPlayerInput(
        bot.id,
        state.dx,
        state.dy,
        ++bot.input.seq,
        Date.now(),
        this.soloSim.simTime,
        primaryAction,
        secondaryAction,
      );
    }
  }

  private handleSoloRoundEnd(): void {
    if (this.soloTimer) clearInterval(this.soloTimer);
    if (this.botAiTimer) clearInterval(this.botAiTimer);

    const standings = this.soloSim!.getRoundStandings();
    const isMatchEnd = this.soloRoundIndex >= this.totalRounds;
    const matchStandings = isMatchEnd ? calculateMatchStandings(this.soloSim!.players) : undefined;

    const myPlayer = this.soloSim!.players.find((p) => p.id === this.myPlayerId);

    this.scene.start('result', {
      mode: 'solo',
      displayName: this.dataPayload.displayName,
      myPlayerId: this.myPlayerId,
      roundIndex: this.soloRoundIndex,
      totalRounds: this.totalRounds,
      roundStandings: standings,
      matchStandings,
      isMatchEnd,
      onSoloNextRound: () => {
        // Go to shop between rounds
        this.scene.start('shop', {
          mode: 'solo',
          displayName: this.dataPayload.displayName,
          myPlayerId: this.myPlayerId,
          cash: myPlayer?.cash ?? 500,
          inventory: myPlayer?.inventory ?? createDefaultInventory(),
          onSoloShopComplete: (updatedCash: number, updatedInventory: ReturnType<typeof createDefaultInventory>) => {
            this.scene.start('game', {
              mode: 'solo',
              displayName: this.dataPayload.displayName,
              myPlayerId: this.myPlayerId,
              soloRoundIndex: this.soloRoundIndex + 1,
              soloCash: updatedCash,
              soloInventory: updatedInventory,
            });
          },
        });
      },
    });
  }

  // --- MULTIPLAYER MODE ---
  private initMultiplayerGame(): void {
    if (!this.socket || !this.dataPayload.startData) return;

    const startData = this.dataPayload.startData;
    const map = generateClassicMine(startData.seed, 4);
    this.currentTiles = map.tiles.map((kind) => ({ kind, durability: kind === 'soil' ? 1000 : 0 }));
    this.renderInitialMap(map.tiles);
    this.roundEndsAt = startData.endsAt;

    (this.socket as unknown as { options: { onMessage: (msg: ServerMessage) => void } }).options.onMessage = (msg: ServerMessage) => {
      this.handleServerMessage(msg);
    };
  }

  private handleServerMessage(msg: ServerMessage): void {
    switch (msg.t) {
      case 's.snapshot':
        this.handleSnapshot(msg);
        break;
      case 's.event':
        this.handleGameEvents(msg.events);
        break;
      case 's.round_result':
        this.scene.start('result', {
          ...this.dataPayload,
          mode: 'multiplayer',
          roundIndex: msg.roundIndex,
          totalRounds: 5,
          roundStandings: msg.standings,
          isMatchEnd: false,
        });
        break;
      case 's.match_result':
        this.scene.start('result', {
          ...this.dataPayload,
          mode: 'multiplayer',
          roundIndex: 5,
          totalRounds: 5,
          matchStandings: msg.standings,
          isMatchEnd: true,
        });
        break;
    }
  }

  private handleSnapshot(snap: Extract<ServerMessage, { t: 's.snapshot' }>): void {
    this.lastServerSeq = snap.serverSeq;

    // Apply Tile Deltas
    for (const delta of snap.changedTiles) {
      const sprite = this.tileSprites.get(delta.index);
      if (sprite) {
        if (delta.kind === 'floor') {
          sprite.destroy();
          this.tileSprites.delete(delta.index);
        } else if (delta.digProgressPermille && delta.digProgressPermille > 300) {
          sprite.setFrame(3); // cracked soil
        }
      }
      if (this.currentTiles[delta.index]) {
        this.currentTiles[delta.index]!.kind = delta.kind;
      }
    }

    // Reconcile Local Player
    const myData = snap.players.find((p) => p.id === this.myPlayerId);
    if (myData) {
      const rec = this.prediction.reconcile(myData.x, myData.y, snap.ackInputSeq, this.currentTiles);
      this.hpText.setText(`${myData.hp}/100`);
      this.hpBar.width = Math.max(0, myData.hp);
      this.hpBar.fillColor = myData.hp > 40 ? 0x2ecc71 : 0xe74c3c;
      this.cashText.setText(`CASH: $${myData.cash}`);
    }

    // Update Remote Players & Interpolation
    for (const p of snap.players) {
      if (p.id !== this.myPlayerId) {
        let interp = this.remoteInterpolations.get(p.id);
        if (!interp) {
          interp = new RemoteEntityInterpolation();
          this.remoteInterpolations.set(p.id, interp);
        }
        interp.pushSnapshot(p.x, p.y, Date.now());
      }
    }

    // Render Dynamic Entities (Explosives, Treasures)
    this.syncEntities(snap.entities);

    // Update Alive Miners Count
    const aliveCount = snap.players.filter((p) => p.alive).length;
    this.aliveText.setText(`MINERS: ${aliveCount}/${snap.players.length}`);
  }

  private renderInitialMap(tiles: string[]): void {
    for (let y = 0; y < MAP_HEIGHT; y++) {
      for (let x = 0; x < MAP_WIDTH; x++) {
        const idx = y * MAP_WIDTH + x;
        const kind = tiles[idx];
        if (kind === 'rock') {
          const s = this.add.sprite(x * TILE_SIZE_PX + 16, y * TILE_SIZE_PX + 16, 'tiles_world', 2);
          this.tileSprites.set(idx, s);
        } else if (kind === 'soil') {
          const s = this.add.sprite(x * TILE_SIZE_PX + 16, y * TILE_SIZE_PX + 16, 'tiles_world', 1);
          this.tileSprites.set(idx, s);
        }
      }
    }
  }

  private syncEntities(entities: EntityView[]): void {
    const currentEntityIds = new Set(entities.map((e) => e.id));

    // Remove obsolete
    for (const [id, sprite] of this.entitySprites) {
      if (!currentEntityIds.has(id)) {
        sprite.destroy();
        this.entitySprites.delete(id);
      }
    }

    // Add or update
    for (const entity of entities) {
      let px = 0;
      let py = 0;
      if (entity.kind === 'projectile' || entity.kind === 'monster') {
        px = (entity.x / WORLD_UNITS_PER_TILE) * TILE_SIZE_PX;
        py = (entity.y / WORLD_UNITS_PER_TILE) * TILE_SIZE_PX;
      } else {
        px = entity.tileX * TILE_SIZE_PX + 16;
        py = entity.tileY * TILE_SIZE_PX + 16;
      }

      let sprite = this.entitySprites.get(entity.id);
      if (!sprite) {
        if (entity.kind === 'explosive') {
          sprite = this.add.sprite(px, py, 'bombs', 0);
          sprite.play('bomb_tick');
          if (entity.isRemote) {
            sprite.setTint(0xff6666);
          } else if (entity.definitionId === 'proximity_mine') {
            sprite.setTint(0x55ff55);
          } else if (entity.definitionId === 'nuke') {
            sprite.setTint(0xffbb00);
          }
        } else if (entity.kind === 'treasure') {
          let frame = 0;
          if (entity.rarity === 'silver') frame = 4;
          else if (entity.rarity === 'ruby') frame = 1;
          else if (entity.rarity === 'diamond') frame = 5;
          else if (entity.rarity === 'chest') frame = 6;
          else frame = 0; // gold / rare

          sprite = this.add.sprite(px, py, 'pickups', frame);
          this.tweens.add({
            targets: sprite,
            y: py - 4,
            duration: 500,
            yoyo: true,
            repeat: -1,
          });
        } else if (entity.kind === 'pickup') {
          let frame = 2; // ammo crate
          if (entity.definitionId === 'med_kit') frame = 3;
          else if (entity.definitionId === 'rocket') frame = 7;
          sprite = this.add.sprite(px, py, 'pickups', frame);
          this.tweens.add({
            targets: sprite,
            y: py - 3,
            duration: 400,
            yoyo: true,
            repeat: -1,
          });
        } else if (entity.kind === 'projectile') {
          sprite = this.add.sprite(px, py, 'projectiles', 0).setDepth(210);
          sprite.setRotation(Math.atan2(entity.vy, entity.vx));
        } else if (entity.kind === 'monster') {
          const frame = entity.monsterKind === 'slime' ? 0 : 2;
          sprite = this.add.sprite(px, py, 'monsters', frame).setDepth(205);
          sprite.play(entity.monsterKind === 'slime' ? 'slime_idle' : 'bat_fly');
        } else if (entity.kind === 'falling_rock') {
          sprite = this.add.sprite(px, py, 'projectiles', 2).setDepth(220);
          this.tweens.add({
            targets: sprite,
            y: py,
            from: py - 32,
            duration: 250,
            ease: 'Bounce.easeOut',
          });
        }
        if (sprite) this.entitySprites.set(entity.id, sprite);
      } else {
        // Entity exists: update positions if mobile
        if (entity.kind === 'projectile') {
          sprite.setPosition(px, py);
          sprite.setRotation(Math.atan2(entity.vy, entity.vx));
        } else if (entity.kind === 'monster') {
          sprite.setPosition(px, py);
        }
      }
    }
  }

  private handleGameEvents(events: GameEvent[]): void {
    for (const ev of events) {
      if (ev.kind === 'tile_changed') {
        const sprite = this.tileSprites.get(ev.index);
        if (sprite && ev.tile === 'floor') {
          sprite.destroy();
          this.tileSprites.delete(ev.index);
          RetroAudio.playDig();
        }
        if (this.currentTiles[ev.index]) {
          this.currentTiles[ev.index]!.kind = ev.tile;
        }
      } else if (ev.kind === 'explosion') {
        const numCells = ev.cells.length;
        if (numCells >= 25) {
          // Nuclear blast
          this.cameras.main.shake(700, 0.04);
          this.cameras.main.flash(300, 255, 255, 255);
          RetroAudio.playExplosion('nuke');
        } else if (numCells >= 13) {
          this.cameras.main.shake(300, 0.02);
          RetroAudio.playExplosion('heavy');
        } else if (numCells >= 8) {
          this.cameras.main.shake(200, 0.015);
          RetroAudio.playExplosion('medium');
        } else {
          this.cameras.main.shake(120, 0.01);
          RetroAudio.playExplosion('small');
        }

        for (const cell of ev.cells) {
          const fx = this.add.sprite(cell.x * TILE_SIZE_PX + 16, cell.y * TILE_SIZE_PX + 16, 'explosions', 0);
          fx.play('explode');
          fx.once('animationcomplete', () => fx.destroy());
        }
      } else if (ev.kind === 'damage') {
        RetroAudio.playHurt();
        const p = this.playerSprites.get(ev.playerId);
        if (p) {
          const txt = this.add.text(p.x, p.y - 20, `-${ev.amount} HP`, {
            fontFamily: 'monospace',
            fontSize: '14px',
            fontStyle: 'bold',
            color: '#ff4757',
          }).setOrigin(0.5).setDepth(300);
          this.tweens.add({
            targets: txt,
            y: txt.y - 24,
            alpha: 0,
            duration: 800,
            onComplete: () => txt.destroy(),
          });
        }
      } else if (ev.kind === 'treasure_collected') {
        RetroAudio.playPickup();
        const p = this.playerSprites.get(ev.playerId);
        if (p) {
          const txt = this.add.text(p.x, p.y - 20, `+$${ev.value}`, {
            fontFamily: 'monospace',
            fontSize: '14px',
            fontStyle: 'bold',
            color: '#2ed573',
          }).setOrigin(0.5).setDepth(300);
          this.tweens.add({
            targets: txt,
            y: txt.y - 24,
            alpha: 0,
            duration: 800,
            onComplete: () => txt.destroy(),
          });
        }
      } else if (ev.kind === 'pickup_collected') {
        RetroAudio.playPickup();
        const p = this.playerSprites.get(ev.playerId);
        if (p) {
          let label = '+2 Bombs!';
          let color = '#f1c40f';
          if (ev.definitionId === 'med_kit') {
            label = '+50 HP Med Kit!';
            color = '#2ecc71';
          } else if (ev.definitionId === 'rocket') {
            label = '+1 Rocket!';
            color = '#e74c3c';
          }
          const txt = this.add.text(p.x, p.y - 20, label, {
            fontFamily: 'monospace',
            fontSize: '14px',
            fontStyle: 'bold',
            color,
          }).setOrigin(0.5).setDepth(300);
          this.tweens.add({
            targets: txt,
            y: txt.y - 24,
            alpha: 0,
            duration: 800,
            onComplete: () => txt.destroy(),
          });
        }
      } else if (ev.kind === 'projectile_fired') {
        RetroAudio.playRocket();
      } else if (ev.kind === 'flame_burst') {
        RetroAudio.playFlame();
        for (const cell of ev.cells) {
          const flame = this.add.sprite(cell.x * TILE_SIZE_PX + 16, cell.y * TILE_SIZE_PX + 16, 'projectiles', 1).setDepth(215);
          this.tweens.add({
            targets: flame,
            alpha: 0,
            scale: 1.3,
            duration: 250,
            onComplete: () => flame.destroy(),
          });
        }
      } else if (ev.kind === 'mine_collapse') {
        RetroAudio.playAlarm();
        this.cameras.main.shake(600, 0.025);
        const banner = this.add.text(this.cameras.main.width / 2, 70, '⚠️ WARNING: MINE COLLAPSE IN PROGRESS! ⚠️', {
          fontFamily: 'monospace',
          fontSize: '18px',
          fontStyle: 'bold',
          color: '#ff3838',
          backgroundColor: '#000000cc',
          padding: { x: 16, y: 6 },
        }).setOrigin(0.5).setDepth(600);
        this.tweens.add({
          targets: banner,
          alpha: 0,
          duration: 3000,
          onComplete: () => banner.destroy(),
        });
      } else if (ev.kind === 'teleported') {
        RetroAudio.playTeleport();
        const p = this.playerSprites.get(ev.playerId);
        if (p) {
          const flash = this.add.circle(p.x, p.y, 24, 0x00ffff, 0.8).setDepth(205);
          this.tweens.add({
            targets: flash,
            scale: 2,
            alpha: 0,
            duration: 300,
            onComplete: () => flash.destroy(),
          });
        }
      }
    }
  }

  update(): void {
    const { state, shouldTransmit } = this.inputController.update();

    if (this.dataPayload.mode === 'solo' && this.soloSim) {
      // Direct local simulation input
      const myPlayer = this.soloSim.players.find((p) => p.id === this.myPlayerId);
      if (myPlayer && myPlayer.alive) {
        if (shouldTransmit && !this.isPaused) {
          this.soloSim.setPlayerInput(
            this.myPlayerId,
            state.dx,
            state.dy,
            ++myPlayer.input.seq,
            Date.now(),
            this.soloSim.simTime,
            state.primary,
            state.secondary,
            state.slot,
          );
        }

        // Render Local Player
        this.updatePlayerSprite(myPlayer.id, myPlayer.x, myPlayer.y, myPlayer.name, 0, myPlayer.alive);

        // Update HUD & Hotbar
        this.hpText.setText(`${myPlayer.hp}/100`);
        this.hpBar.width = Math.max(0, myPlayer.hp);
        this.hpBar.fillColor = myPlayer.hp > 40 ? 0x2ecc71 : 0xe74c3c;
        this.cashText.setText(`CASH: $${myPlayer.cash}`);
        this.updateHotbar(myPlayer.inventory, state.slot);
      }

      // Render Bots
      for (let i = 1; i < this.soloSim.players.length; i++) {
        const bot = this.soloSim.players[i]!;
        this.updatePlayerSprite(bot.id, bot.x, bot.y, bot.name, i, bot.alive);
      }

      // Sync Entities for Solo
      const snap = this.soloSim.getSnapshot(this.myPlayerId);
      this.syncEntities(snap.entities);

      // Apply changed tiles (cracking / opening) in Solo
      for (const delta of snap.changedTiles) {
        const sprite = this.tileSprites.get(delta.index);
        if (sprite) {
          if (delta.kind === 'floor') {
            sprite.destroy();
            this.tileSprites.delete(delta.index);
          } else if (delta.digProgressPermille && delta.digProgressPermille > 150) {
            sprite.setFrame(3); // cracked soil
          }
        }
        if (this.currentTiles[delta.index]) {
          this.currentTiles[delta.index]!.kind = delta.kind;
        }
      }

      const aliveCount = this.soloSim.players.filter((p) => p.alive).length;
      this.aliveText.setText(`MINERS: ${aliveCount}/${this.soloSim.players.length}`);
    } else if (this.socket) {
      // Multiplayer prediction
      if (shouldTransmit) {
        const seq = this.socket.sendInput(state.dx, state.dy, state.slot, state.primary, state.secondary);
        this.prediction.predictMove(state.dx, state.dy, 50, seq, this.currentTiles);
      }

      // Render Predicted Local Player
      this.updatePlayerSprite(this.myPlayerId, this.prediction.predictedX, this.prediction.predictedY, this.dataPayload.displayName, 0, true);

      // Render Interpolated Remote Players
      for (const [id, interp] of this.remoteInterpolations) {
        const pos = interp.getInterpolatedPosition();
        this.updatePlayerSprite(id, pos.x, pos.y, id, 1, true);
      }
    }

    // Update Round Timer in HUD
    const remainingMs = Math.max(0, this.roundEndsAt - Date.now());
    const min = Math.floor(remainingMs / 60000).toString().padStart(2, '0');
    const sec = Math.floor((remainingMs % 60000) / 1000).toString().padStart(2, '0');
    this.timerText.setText(`TIME: ${min}:${sec}`);

    // Update Debug Overlay
    this.debugOverlay.update({
      fps: this.game.loop.actualFps,
      rtt: this.socket?.clockSync.rtt ?? 0,
      serverSeq: this.lastServerSeq,
      predictionErrorPx: 0,
      entitiesCount: this.entitySprites.size,
      phase: 'playing',
    });
  }

  private updatePlayerSprite(id: string, worldX: number, worldY: number, name: string, colorIdx: number, alive: boolean): void {
    const px = (worldX / WORLD_UNITS_PER_TILE) * TILE_SIZE_PX;
    const py = (worldY / WORLD_UNITS_PER_TILE) * TILE_SIZE_PX;

    let sprite = this.playerSprites.get(id);
    let label = this.playerLabels.get(id);

    if (!sprite) {
      sprite = this.add.sprite(px, py, 'miner', 0);
      sprite.setDepth(200);
      sprite.setTint(AssetRegistry.getPlayerColor(colorIdx));
      this.playerSprites.set(id, sprite);

      label = this.add.text(px, py - 24, name, {
        fontFamily: 'monospace',
        fontSize: '12px',
        color: '#ffffff',
        backgroundColor: '#00000088',
        padding: { x: 4, y: 2 },
      }).setOrigin(0.5).setDepth(201);
      this.playerLabels.set(id, label);
    }

    if (!alive) {
      sprite.setAlpha(0.3);
      sprite.setAngle(90);
      label?.setText(`${name} (💀)`);
    } else {
      sprite.setAlpha(1.0);
      sprite.setAngle(0);

      const prev = this.playerPrevPositions.get(id);
      if (prev) {
        const dx = px - prev.x;
        const dy = py - prev.y;
        if (Math.abs(dx) > 0.4 || Math.abs(dy) > 0.4) {
          if (Math.abs(dx) > Math.abs(dy)) {
            sprite.play(dx > 0 ? 'miner_right' : 'miner_left', true);
          } else {
            sprite.play(dy > 0 ? 'miner_down' : 'miner_up', true);
          }
        } else {
          sprite.stop();
        }
      }
      this.playerPrevPositions.set(id, { x: px, y: py });

      sprite.setPosition(px, py);
      label?.setPosition(px, py - 24);
    }
  }

  shutdown(): void {
    if (this.soloTimer) clearInterval(this.soloTimer);
    if (this.botAiTimer) clearInterval(this.botAiTimer);
    if (this.pauseContainer) {
      this.pauseContainer.destroy();
      this.pauseContainer = null;
    }
    this.debugOverlay.destroy();
  }
}
