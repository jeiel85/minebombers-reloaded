import Phaser from 'phaser';
import type { GameSocket } from '../net/GameSocket';
import type { InventoryState, ServerMessage } from '@minebombers/shared';
import { EQUIPMENT } from '@minebombers/shared';
import { RetroAudio } from '../audio/RetroAudio';

export interface ShopSceneData {
  mode?: 'multiplayer' | 'solo';
  roomCode?: string;
  displayName?: string;
  socket?: GameSocket;
  myPlayerId?: string;
  cash?: number;
  inventory?: InventoryState;
  selectedMap?: string;
  botCount?: number;
  botDifficulty?: 'easy' | 'normal' | 'hardcore';
  persistedBots?: Array<{ id: string; name: string; cash: number; inventory: InventoryState }>;
  shopData?: Extract<ServerMessage, { t: 's.shop' }>;
  onSoloShopComplete?: (cash: number, inventory: InventoryState) => void;
}

interface ShopItemConfig {
  id: string;
  name: string;
  cost: number;
  desc: string;
  iconFrame: number;
  isUpgrade?: boolean;
  category: 'explosives' | 'weapons' | 'gear';
}

const ALL_SHOP_ITEMS: ShopItemConfig[] = [
  // Explosives
  { id: 'small_charge', name: 'Small Bomb', cost: 120, desc: 'Radius 2, 65 DMG. Affordable & quick soil excavation.', iconFrame: 0, category: 'explosives' },
  { id: 'dynamite', name: 'Dynamite Bundle', cost: 160, desc: 'Radius 3, 85 DMG. Solid medium blast with short fuse.', iconFrame: 1, category: 'explosives' },
  { id: 'heavy_charge', name: 'Heavy Bomb', cost: 300, desc: 'Radius 4, 100 DMG. Shatters rock crust and penetrates dirt.', iconFrame: 2, category: 'explosives' },
  { id: 'remote_bomb', name: 'Remote Bomb', cost: 320, desc: 'Radius 3, 95 DMG. Detonates on command via [F] key.', iconFrame: 3, category: 'explosives' },
  { id: 'proximity_mine', name: 'Landmine', cost: 220, desc: 'Radius 2, 100 DMG. Concealed trap triggered by enemy steps.', iconFrame: 4, category: 'explosives' },
  { id: 'nuke', name: 'Nuclear Warhead', cost: 1200, desc: 'Radius 8, 200 DMG. Apocalyptic screen-clearing blast!', iconFrame: 7, category: 'explosives' },

  // Weapons
  { id: 'rocket', name: 'Mini-Rocket', cost: 180, desc: 'High-speed straight-line rocket penetrating dirt tunnels.', iconFrame: 5, category: 'weapons' },
  { id: 'flamethrower', name: 'Flamethrower', cost: 350, desc: '3-tile linear scorch melting soil and incinerating enemies.', iconFrame: 6, category: 'weapons' },

  // Gear & Utility
  { id: 'pickaxe_2', name: 'Steel Pickaxe II', cost: 500, desc: '1.5x Mining Speed. Permanent match passive upgrade.', iconFrame: 0, isUpgrade: true, category: 'gear' },
  { id: 'power_drill', name: 'Pneumatic Drill', cost: 1000, desc: '2.5x Super Dig Speed! Shreds through cavern stone.', iconFrame: 0, isUpgrade: true, category: 'gear' },
  { id: 'kevlar_armor', name: 'Kevlar Armor', cost: 450, desc: '40% Blast Damage Reduction. Permanent passive upgrade.', iconFrame: 0, isUpgrade: true, category: 'gear' },
  { id: 'teleport', name: 'Emergency Teleporter', cost: 250, desc: 'Instantly warps user to a random safe mine chamber.', iconFrame: 0, category: 'gear' },
  { id: 'med_kit', name: 'Field Med Kit', cost: 150, desc: 'Restores +50 HP immediately upon use.', iconFrame: 0, category: 'gear' },
];

export class ShopScene extends Phaser.Scene {
  private dataPayload!: ShopSceneData;
  private cash = 500;
  private inventory: InventoryState = {
    selectedSlot: 0,
    items: {},
    upgrades: { pickaxe_1: 1 },
  };
  private closesAt = 0;
  private isReady = false;
  private activeCategory: 'explosives' | 'weapons' | 'gear' = 'explosives';

  private cashText!: Phaser.GameObjects.Text;
  private timerText!: Phaser.GameObjects.Text;
  private readyBtn!: Phaser.GameObjects.Text;
  private feedbackText!: Phaser.GameObjects.Text;
  private tabButtons: Phaser.GameObjects.Text[] = [];
  private itemCardContainers: Phaser.GameObjects.Container[] = [];
  private itemStatusTexts = new Map<string, Phaser.GameObjects.Text>();

  constructor() {
    super('shop');
  }

  init(data: ShopSceneData): void {
    this.dataPayload = data;
    if (data.shopData) {
      this.cash = data.shopData.cash;
      this.inventory = data.shopData.inventory;
      this.closesAt = data.shopData.closesAt;
    } else {
      this.cash = data.cash ?? 500;
      this.inventory = data.inventory ?? { selectedSlot: 0, items: {}, upgrades: { pickaxe_1: 1 } };
      this.closesAt = Date.now() + 25_000;
    }
  }

  create(): void {
    const width = this.cameras.main.width;
    const height = this.cameras.main.height;

    // Stream 1995 Scream Tracker 3 Huipentuja BGM in Shop
    RetroAudio.playBGM('huippe');

    // Header Background
    this.add.rectangle(width / 2, 40, width, 80, 0x111620, 0.95);

    // Title
    this.add.text(width / 2, 30, 'MINE BOMBERS EQUIPMENT SHOP', {
      fontFamily: 'monospace',
      fontSize: '24px',
      fontStyle: 'bold',
      color: '#f39c12',
    }).setOrigin(0.5);

    // Cash Display
    this.cashText = this.add.text(60, 30, `CASH: $${this.cash}`, {
      fontFamily: 'monospace',
      fontSize: '20px',
      fontStyle: 'bold',
      color: '#2ecc71',
    }).setOrigin(0, 0.5);

    // Timer Display
    this.timerText = this.add.text(width - 60, 30, '25s', {
      fontFamily: 'monospace',
      fontSize: '20px',
      fontStyle: 'bold',
      color: '#e74c3c',
    }).setOrigin(1, 0.5);

    // Solo Battle Info
    if (this.dataPayload.mode === 'solo') {
      const bots = this.dataPayload.botCount ?? 3;
      const diff = (this.dataPayload.botDifficulty ?? 'normal').toUpperCase();
      this.add.text(width / 2, 54, `⚔️ SOLO BATTLE: VS ${bots} BOTS [${diff} AI]`, {
        fontFamily: 'monospace',
        fontSize: '12px',
        color: '#00ffcc',
      }).setOrigin(0.5);
    }

    // Feedback notification banner
    this.feedbackText = this.add.text(width / 2, 70, '', {
      fontFamily: 'monospace',
      fontSize: '13px',
      color: '#f1c40f',
    }).setOrigin(0.5);

    // Category Tabs
    this.createCategoryTabs(width);

    // Render initial category cards
    this.renderCategoryCards();

    // Ready / Done Shopping Button
    this.readyBtn = this.add.text(width / 2, height - 38, 'READY FOR NEXT ROUND', {
      fontFamily: 'monospace',
      fontSize: '18px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: '#2980b9',
      padding: { x: 36, y: 10 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    this.readyBtn.on('pointerdown', () => {
      RetroAudio.playClick();
      this.isReady = !this.isReady;
      this.readyBtn.setText(this.isReady ? 'WAITING FOR OTHERS...' : 'READY FOR NEXT ROUND');
      this.readyBtn.setBackgroundColor(this.isReady ? '#7f8c8d' : '#2980b9');

      if (this.dataPayload.mode === 'solo') {
        this.dataPayload.onSoloShopComplete?.(this.cash, this.inventory);
      } else if (this.dataPayload.socket) {
        this.dataPayload.socket.setReady(this.isReady);
      }
    });

    // Socket message listening if multiplayer
    if (this.dataPayload.socket) {
      (this.dataPayload.socket as unknown as { options: { onMessage: (msg: ServerMessage) => void } }).options.onMessage = (msg: ServerMessage) => {
        this.handleServerMessage(msg);
      };
    }
  }

  private createCategoryTabs(width: number): void {
    const tabs: Array<{ category: 'explosives' | 'weapons' | 'gear'; label: string }> = [
      { category: 'explosives', label: '💣 EXPLOSIVES' },
      { category: 'weapons', label: '🚀 WEAPONS' },
      { category: 'gear', label: '🛡️ GEAR & UTILITY' },
    ];

    const tabWidth = 220;
    const totalW = tabs.length * tabWidth;
    const startX = (width - totalW) / 2 + tabWidth / 2;
    const tabY = 96;

    this.tabButtons = [];

    tabs.forEach((tab, index) => {
      const tx = startX + index * tabWidth;
      const isActive = tab.category === this.activeCategory;

      const btn = this.add.text(tx, tabY, tab.label, {
        fontFamily: 'monospace',
        fontSize: '15px',
        fontStyle: 'bold',
        color: isActive ? '#f1c40f' : '#bdc3c7',
        backgroundColor: isActive ? '#2c3e50' : '#1e272e',
        padding: { x: 18, y: 8 },
      }).setOrigin(0.5).setInteractive({ useHandCursor: true });

      btn.on('pointerdown', () => {
        if (this.activeCategory !== tab.category) {
          RetroAudio.playClick();
          this.activeCategory = tab.category;
          this.updateTabStyles();
          this.renderCategoryCards();
        }
      });

      this.tabButtons.push(btn);
    });
  }

  private updateTabStyles(): void {
    const categories: Array<'explosives' | 'weapons' | 'gear'> = ['explosives', 'weapons', 'gear'];
    this.tabButtons.forEach((btn, index) => {
      const cat = categories[index];
      const isActive = cat === this.activeCategory;
      btn.setColor(isActive ? '#f1c40f' : '#bdc3c7');
      btn.setBackgroundColor(isActive ? '#2c3e50' : '#1e272e');
    });
  }

  private renderCategoryCards(): void {
    // Destroy previous cards
    for (const container of this.itemCardContainers) {
      container.destroy();
    }
    this.itemCardContainers = [];
    this.itemStatusTexts.clear();

    const width = this.cameras.main.width;
    const items = ALL_SHOP_ITEMS.filter((i) => i.category === this.activeCategory);

    items.forEach((item, index) => {
      const cardY = 150 + index * 84;
      const bg = this.add.rectangle(width / 2, cardY, width - 120, 72, 0x1a202c, 0.95);
      bg.setStrokeStyle(1, 0x34495e);

      // Icon
      const icon = this.add.sprite(90, cardY, 'ui_icons', item.iconFrame).setScale(1.1);

      // Title
      const title = this.add.text(125, cardY - 22, item.name, {
        fontFamily: 'monospace',
        fontSize: '17px',
        fontStyle: 'bold',
        color: '#ffffff',
      });

      // Description
      const desc = this.add.text(125, cardY + 5, item.desc, {
        fontFamily: 'monospace',
        fontSize: '12px',
        color: '#95a5a6',
      });

      // Status (Price & Owned)
      const count = item.isUpgrade
        ? (this.inventory.upgrades[item.id] ?? 0)
        : (this.inventory.items[item.id] ?? 0);

      const status = this.add.text(width - 280, cardY - 6, `$${item.cost} | Owned: ${count}`, {
        fontFamily: 'monospace',
        fontSize: '15px',
        fontStyle: 'bold',
        color: '#f1c40f',
      });
      this.itemStatusTexts.set(item.id, status);

      // Buy Button
      const isMaxed = item.isUpgrade && count >= 1;
      const buyBtn = this.add.text(width - 120, cardY, isMaxed ? 'OWNED' : 'BUY', {
        fontFamily: 'monospace',
        fontSize: '15px',
        fontStyle: 'bold',
        color: '#ffffff',
        backgroundColor: isMaxed ? '#7f8c8d' : '#27ae60',
        padding: { x: 18, y: 8 },
      }).setOrigin(0.5);

      if (!isMaxed) {
        buyBtn.setInteractive({ useHandCursor: true });
        buyBtn.on('pointerdown', () => {
          this.handleBuy(item.id, item.cost, item.isUpgrade);
        });
      }

      const card = this.add.container(0, 0, [bg, icon, title, desc, status, buyBtn]);
      this.itemCardContainers.push(card);
    });
  }

  update(): void {
    const remainingSeconds = Math.max(0, Math.ceil((this.closesAt - Date.now()) / 1000));
    this.timerText.setText(`${remainingSeconds}s`);

    if (remainingSeconds <= 0 && this.dataPayload.mode === 'solo' && !this.isReady) {
      this.isReady = true;
      this.dataPayload.onSoloShopComplete?.(this.cash, this.inventory);
    }
  }

  private handleBuy(equipmentId: string, cost: number, isUpgrade?: boolean): void {
    if (this.dataPayload.mode === 'solo') {
      if (this.cash < cost) {
        RetroAudio.playHurt();
        this.showFeedback(`Not enough cash for ${equipmentId}!`, '#e74c3c');
        return;
      }
      if (isUpgrade && (this.inventory.upgrades[equipmentId] ?? 0) >= 1) {
        this.showFeedback('Already permanently owned!', '#e67e22');
        return;
      }

      this.cash -= cost;
      if (isUpgrade) {
        this.inventory.upgrades[equipmentId] = (this.inventory.upgrades[equipmentId] ?? 0) + 1;
      } else {
        this.inventory.items[equipmentId] = (this.inventory.items[equipmentId] ?? 0) + 1;
      }

      RetroAudio.playBuy();
      this.showFeedback(`Purchased 1x ${equipmentId}!`, '#2ecc71');
      this.updateDisplay();
    } else if (this.dataPayload.socket) {
      this.dataPayload.socket.buy(equipmentId, 1);
    }
  }

  private showFeedback(msg: string, color: string): void {
    this.feedbackText.setText(msg);
    this.feedbackText.setColor(color);
    this.tweens.add({
      targets: this.feedbackText,
      alpha: 1,
      duration: 100,
      yoyo: true,
      hold: 1200,
    });
  }

  private handleServerMessage(msg: ServerMessage): void {
    if (msg.t === 's.purchase_result') {
      if (msg.accepted) {
        RetroAudio.playBuy();
        this.cash = msg.cash;
        this.inventory = msg.inventory;
        this.showFeedback('Purchase successful!', '#2ecc71');
        this.updateDisplay();
      } else {
        RetroAudio.playHurt();
        this.showFeedback(`Purchase failed: ${msg.code}`, '#e74c3c');
      }
    } else if (msg.t === 's.start') {
      this.scene.start('game', {
        mode: 'multiplayer',
        ...this.dataPayload,
        startData: msg,
      });
    }
  }

  private updateDisplay(): void {
    this.cashText.setText(`CASH: $${this.cash}`);
    this.renderCategoryCards();
  }
}
