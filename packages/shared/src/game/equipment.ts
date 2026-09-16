export interface EquipmentDef {
  id: string;
  kind: 'tool' | 'explosive' | 'mine' | 'consumable';
  cost: number;
  maxOwned?: number;
  maxActive?: number;
  cooldownMs?: number;
  fuseMs?: number;
  blastRadiusTiles?: number;
  damage?: number;
  digMultiplier?: number;
  heal?: number;
  penetration?: number;
}

export const EQUIPMENT = {
  pickaxe_1: {
    id: 'pickaxe_1', kind: 'tool', cost: 0, digMultiplier: 1.0,
  },
  pickaxe_2: {
    id: 'pickaxe_2', kind: 'tool', cost: 700, maxOwned: 1, digMultiplier: 1.45,
  },
  small_charge: {
    id: 'small_charge', kind: 'explosive', cost: 120, maxActive: 2,
    fuseMs: 1800, blastRadiusTiles: 2, damage: 70, penetration: 0,
  },
  heavy_charge: {
    id: 'heavy_charge', kind: 'explosive', cost: 300, maxActive: 1,
    fuseMs: 2400, blastRadiusTiles: 4, damage: 100, penetration: 0,
  },
  proximity_mine: {
    id: 'proximity_mine', kind: 'mine', cost: 250, maxActive: 2,
    cooldownMs: 1000, blastRadiusTiles: 1, damage: 100,
  },
  med_kit: {
    id: 'med_kit', kind: 'consumable', cost: 200, heal: 40,
  },
} as const satisfies Record<string, EquipmentDef>;

export type EquipmentId = keyof typeof EQUIPMENT;
