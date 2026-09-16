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
  type GameEvent,
  type ServerMessage,
  type SimTileState,
} from '@minebombers/shared';
import { AssetRegistry } from '../game/AssetRegistry';
import { InputController } from '../game/InputController';
import { ClientPrediction } from '../game/Prediction';
import { RemoteEntityInterpolation } from '../game/Interpolation';
import { DebugOverlay } from '../ui/DebugOverlay';

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
  private entitySprites = new Map<string, Phaser.GameObjects.Sprite>();

  // HUD
  private hudBg!: Phaser.GameObjects.Rectangle;
  private hpText!: Phaser.GameObjects.Text;
  private hpBar!: Phaser.GameObjects.Rectangle;
  private cashText!: Phaser.GameObjects.Text;
  private ammoText!: Phaser.GameObjects.Text;
  private timerText!: Phaser.GameObjects.Text;
  private aliveText!: Phaser.GameObjects.Text;

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
  }

  create(): void {
    const arenaWidth = MAP_WIDTH * TILE_SIZE_PX; // 992
    const arenaHeight = MAP_HEIGHT * TILE_SIZE_PX; // 736

    // 1. Floor Background Layer
    this.floorLayer = this.add.tileSprite(0, 0, arenaWidth, arenaHeight, 'tiles_world', 0).setOrigin(0, 0);

    // Initialize Input & Debug
    this.inputController = new InputController(this);
    this.debugOverlay = new DebugOverlay(this);

    // Create HUD
    this.createHUD(arenaWidth);

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
    this.cashText = this.add.text(260, 12, 'CASH: $500', {
      fontFamily: 'monospace',
      fontSize: '16px',
      fontStyle: 'bold',
      color: '#2ecc71',
    }).setDepth(501);

    // Ammo / Equipped
    this.ammoText = this.add.text(420, 12, '💣 Small Charge: 2', {
      fontFamily: 'monospace',
      fontSize: '15px',
      color: '#f1c40f',
    }).setDepth(501);

    // Timer
    this.timerText = this.add.text(700, 12, 'TIME: 05:00', {
      fontFamily: 'monospace',
      fontSize: '16px',
      fontStyle: 'bold',
      color: '#e74c3c',
    }).setDepth(501);

    // Alive
    this.aliveText = this.add.text(860, 12, 'MINERS: 4/4', {
      fontFamily: 'monospace',
      fontSize: '15px',
      color: '#3498db',
    }).setDepth(501);
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
      if (!this.soloSim) return;
      const events = this.soloSim.step(50);
      this.handleGameEvents(events);

      // Check for round end
      if (this.soloSim.phase === 'round_result') {
        this.handleSoloRoundEnd();
      }
    }, 50);

    // Bot AI Decision Loop (~400ms)
    this.botAiTimer = setInterval(() => {
      this.runBotAI();
    }, 400);
  }

  private runBotAI(): void {
    if (!this.soloSim || this.soloSim.phase !== 'playing') return;

    const directions: Array<[-1 | 0 | 1, -1 | 0 | 1]> = [
      [0, -1], [0, 1], [-1, 0], [1, 0],
    ];

    for (let i = 1; i < this.soloSim.players.length; i++) {
      const bot = this.soloSim.players[i]!;
      if (!bot.alive) continue;

      // Random cardinal movement toward treasure or soil
      const dir = directions[Math.floor(Math.random() * directions.length)]!;
      const shouldDropBomb = Math.random() < 0.05; // 5% chance to drop bomb

      this.soloSim.setPlayerInput(bot.id, dir[0], dir[1], ++bot.input.seq, Date.now(), this.soloSim.simTime, shouldDropBomb);
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

  private syncEntities(entities: Array<{ id: string; kind: string; tileX: number; tileY: number }>): void {
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
      const px = entity.tileX * TILE_SIZE_PX + 16;
      const py = entity.tileY * TILE_SIZE_PX + 16;

      let sprite = this.entitySprites.get(entity.id);
      if (!sprite) {
        if (entity.kind === 'explosive') {
          sprite = this.add.sprite(px, py, 'bombs', 0);
          sprite.play('bomb_tick');
        } else if (entity.kind === 'treasure') {
          sprite = this.add.sprite(px, py, 'pickups', 0);
          this.tweens.add({
            targets: sprite,
            y: py - 4,
            duration: 500,
            yoyo: true,
            repeat: -1,
          });
        }
        if (sprite) this.entitySprites.set(entity.id, sprite);
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
        }
        if (this.currentTiles[ev.index]) {
          this.currentTiles[ev.index]!.kind = ev.tile;
        }
      } else if (ev.kind === 'explosion') {
        // Spawn animated explosion segments and camera shake
        this.cameras.main.shake(150, 0.01);
        for (const cell of ev.cells) {
          const fx = this.add.sprite(cell.x * TILE_SIZE_PX + 16, cell.y * TILE_SIZE_PX + 16, 'explosions', 0);
          fx.play('explode');
          fx.once('animationcomplete', () => fx.destroy());
        }
      } else if (ev.kind === 'damage') {
        const p = this.playerSprites.get(ev.playerId);
        if (p) {
          // Floating damage text
          const txt = this.add.text(p.x, p.y - 20, `-${ev.amount} HP`, {
            fontFamily: 'monospace',
            fontSize: '14px',
            fontStyle: 'bold',
            color: '#ff4757',
          }).setOrigin(0.5);
          this.tweens.add({
            targets: txt,
            y: txt.y - 24,
            alpha: 0,
            duration: 800,
            onComplete: () => txt.destroy(),
          });
        }
      } else if (ev.kind === 'treasure_collected') {
        const p = this.playerSprites.get(ev.playerId);
        if (p) {
          const txt = this.add.text(p.x, p.y - 20, `+$${ev.value}`, {
            fontFamily: 'monospace',
            fontSize: '14px',
            fontStyle: 'bold',
            color: '#2ed573',
          }).setOrigin(0.5);
          this.tweens.add({
            targets: txt,
            y: txt.y - 24,
            alpha: 0,
            duration: 800,
            onComplete: () => txt.destroy(),
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
        if (shouldTransmit) {
          this.soloSim.setPlayerInput(
            this.myPlayerId,
            state.dx,
            state.dy,
            ++myPlayer.input.seq,
            Date.now(),
            this.soloSim.simTime,
            state.primary,
            state.secondary,
          );
        }

        // Render Local Player
        this.updatePlayerSprite(myPlayer.id, myPlayer.x, myPlayer.y, myPlayer.name, 0, myPlayer.alive);

        // Update HUD
        this.hpText.setText(`${myPlayer.hp}/100`);
        this.hpBar.width = Math.max(0, myPlayer.hp);
        this.hpBar.fillColor = myPlayer.hp > 40 ? 0x2ecc71 : 0xe74c3c;
        this.cashText.setText(`CASH: $${myPlayer.cash}`);
        const ammo = myPlayer.inventory.items['small_charge'] ?? 0;
        this.ammoText.setText(`💣 Small Charge: ${ammo}`);
      }

      // Render Bots
      for (let i = 1; i < this.soloSim.players.length; i++) {
        const bot = this.soloSim.players[i]!;
        this.updatePlayerSprite(bot.id, bot.x, bot.y, bot.name, i, bot.alive);
      }

      // Sync Entities for Solo
      const snap = this.soloSim.getSnapshot(this.myPlayerId);
      this.syncEntities(snap.entities);

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
      sprite.setPosition(px, py);
      label?.setPosition(px, py - 24);
    }
  }

  shutdown(): void {
    if (this.soloTimer) clearInterval(this.soloTimer);
    if (this.botAiTimer) clearInterval(this.botAiTimer);
    this.debugOverlay.destroy();
  }
}
