import Phaser from 'phaser';
import type { GameSocket } from '../net/GameSocket';
import type { InventoryState, ServerMessage } from '@minebombers/shared';
import { EQUIPMENT } from '@minebombers/shared';

export interface ShopSceneData {
  mode?: 'multiplayer' | 'solo';
  roomCode?: string;
  displayName?: string;
  socket?: GameSocket;
  myPlayerId?: string;
  cash?: number;
  inventory?: InventoryState;
  shopData?: Extract<ServerMessage, { t: 's.shop' }>;
  onSoloShopComplete?: (cash: number, inventory: InventoryState) => void;
}

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

  private cashText!: Phaser.GameObjects.Text;
  private timerText!: Phaser.GameObjects.Text;
  private readyBtn!: Phaser.GameObjects.Text;
  private itemCardTexts = new Map<string, Phaser.GameObjects.Text>();

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
      this.closesAt = Date.now() + 20_000;
    }
  }

  create(): void {
    const width = this.cameras.main.width;
    const height = this.cameras.main.height;

    // Header
    this.add.text(width / 2, 45, 'EQUIPMENT SHOP', {
      fontFamily: 'monospace',
      fontSize: '32px',
      fontStyle: 'bold',
      color: '#f39c12',
    }).setOrigin(0.5);

    this.cashText = this.add.text(80, 45, `CASH: $${this.cash}`, {
      fontFamily: 'monospace',
      fontSize: '22px',
      fontStyle: 'bold',
      color: '#2ecc71',
    }).setOrigin(0, 0.5);

    this.timerText = this.add.text(width - 80, 45, '20s', {
      fontFamily: 'monospace',
      fontSize: '22px',
      fontStyle: 'bold',
      color: '#e74c3c',
    }).setOrigin(1, 0.5);

    // Items list
    const shopItems = [
      { id: 'small_charge', name: 'Small Charge', cost: 120, desc: 'Radius 2, 70 DMG. Destroys soil.', iconFrame: 0 },
      { id: 'heavy_charge', name: 'Heavy Charge', cost: 300, desc: 'Radius 4, 100 DMG. Huge blast!', iconFrame: 1 },
      { id: 'proximity_mine', name: 'Proximity Mine', cost: 250, desc: 'Radius 1, 100 DMG. Proximity trigger.', iconFrame: 2 },
      { id: 'pickaxe_2', name: 'Pickaxe II', cost: 700, desc: '1.45x Dig Speed. Permanent upgrade.', iconFrame: 0, isUpgrade: true },
      { id: 'med_kit', name: 'Med Kit', cost: 200, desc: 'Heals 40 HP immediately.', iconFrame: 2 },
    ];

    shopItems.forEach((item, index) => {
      const cardY = 120 + index * 95;
      const bg = this.add.rectangle(width / 2, cardY, width - 140, 80, 0x1f242d, 0.95);
      bg.setStrokeStyle(1, 0x34495e);

      // Name & Description
      this.add.text(120, cardY - 24, item.name, {
        fontFamily: 'monospace',
        fontSize: '18px',
        fontStyle: 'bold',
        color: '#ffffff',
      });

      this.add.text(120, cardY + 4, item.desc, {
        fontFamily: 'monospace',
        fontSize: '13px',
        color: '#95a5a6',
      });

      // Price & Owned status
      const count = item.isUpgrade
        ? (this.inventory.upgrades[item.id] ?? 0)
        : (this.inventory.items[item.id] ?? 0);

      const status = this.add.text(width - 320, cardY - 8, `$${item.cost} | Owned: ${count}`, {
        fontFamily: 'monospace',
        fontSize: '16px',
        color: '#f1c40f',
      });
      this.itemCardTexts.set(item.id, status);

      // Buy Button
      const buyBtn = this.add.text(width - 150, cardY, 'BUY', {
        fontFamily: 'monospace',
        fontSize: '16px',
        fontStyle: 'bold',
        color: '#ffffff',
        backgroundColor: '#27ae60',
        padding: { x: 18, y: 10 },
      }).setOrigin(0.5).setInteractive({ useHandCursor: true });

      buyBtn.on('pointerdown', () => {
        this.handleBuy(item.id, item.cost, item.isUpgrade);
      });
    });

    // Ready / Done Shopping Button
    this.readyBtn = this.add.text(width / 2, height - 40, 'READY FOR NEXT ROUND', {
      fontFamily: 'monospace',
      fontSize: '18px',
      fontStyle: 'bold',
      color: '#ffffff',
      backgroundColor: '#2980b9',
      padding: { x: 32, y: 12 },
    }).setOrigin(0.5).setInteractive({ useHandCursor: true });

    this.readyBtn.on('pointerdown', () => {
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
        alert('Not enough cash!');
        return;
      }
      if (isUpgrade && (this.inventory.upgrades[equipmentId] ?? 0) >= 1) {
        alert('Already owned!');
        return;
      }
      this.cash -= cost;
      if (isUpgrade) {
        this.inventory.upgrades[equipmentId] = (this.inventory.upgrades[equipmentId] ?? 0) + 1;
      } else {
        this.inventory.items[equipmentId] = (this.inventory.items[equipmentId] ?? 0) + 1;
      }
      this.updateDisplay();
    } else if (this.dataPayload.socket) {
      this.dataPayload.socket.buy(equipmentId, 1);
    }
  }

  private handleServerMessage(msg: ServerMessage): void {
    if (msg.t === 's.purchase_result') {
      if (msg.accepted) {
        this.cash = msg.cash;
        this.inventory = msg.inventory;
        this.updateDisplay();
      } else {
        alert(`Purchase failed: ${msg.code}`);
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
    const items = ['small_charge', 'heavy_charge', 'proximity_mine', 'pickaxe_2', 'med_kit'];
    for (const id of items) {
      const isUpgrade = id === 'pickaxe_2';
      const count = isUpgrade
        ? (this.inventory.upgrades[id] ?? 0)
        : (this.inventory.items[id] ?? 0);
      const cost = (EQUIPMENT as Record<string, { cost: number }>)[id]?.cost ?? 0;
      const text = this.itemCardTexts.get(id);
      if (text) {
        text.setText(`$${cost} | Owned: ${count}`);
      }
    }
  }
}
