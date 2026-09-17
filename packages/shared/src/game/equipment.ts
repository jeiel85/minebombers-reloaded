export interface EquipmentDef {
  id: string;
  name: string;
  category: 'explosives' | 'weapons' | 'gear';
  kind: 'tool' | 'explosive' | 'mine' | 'projectile' | 'special' | 'consumable' | 'passive';
  cost: number;
  description: string;
  maxOwned?: number;
  maxActive?: number;
  cooldownMs?: number;
  fuseMs?: number;
  blastRadiusTiles?: number;
  damage?: number;
  digMultiplier?: number;
  damageReduction?: number;
  heal?: number;
  penetration?: number;
}

export const EQUIPMENT = {
  // Explosives
  small_charge: {
    id: 'small_charge',
    name: 'Small Bomb',
    category: 'explosives',
    kind: 'explosive',
    cost: 120,
    maxActive: 3,
    fuseMs: 1600,
    blastRadiusTiles: 2,
    damage: 65,
    penetration: 0,
    description: 'Compact bomb. Cheap and effective for clearing soil.',
  },
  dynamite: {
    id: 'dynamite',
    name: 'Dynamite',
    category: 'explosives',
    kind: 'explosive',
    cost: 160,
    maxActive: 2,
    fuseMs: 2000,
    blastRadiusTiles: 3,
    damage: 85,
    penetration: 0,
    description: 'Standard medium dynamite bundle with strong 3-tile blast.',
  },
  heavy_charge: {
    id: 'heavy_charge',
    name: 'Heavy Bomb',
    category: 'explosives',
    kind: 'explosive',
    cost: 300,
    maxActive: 1,
    fuseMs: 2400,
    blastRadiusTiles: 4,
    damage: 100,
    penetration: 1,
    description: 'Heavy bomb. Shatters 4-tile radius and penetrates soil.',
  },
  remote_bomb: {
    id: 'remote_bomb',
    name: 'Remote Bomb',
    category: 'explosives',
    kind: 'explosive',
    cost: 320,
    maxActive: 3,
    blastRadiusTiles: 3,
    damage: 95,
    description: 'Detonated on command using Secondary Action (F key).',
  },
  proximity_mine: {
    id: 'proximity_mine',
    name: 'Landmine',
    category: 'explosives',
    kind: 'mine',
    cost: 220,
    maxActive: 3,
    cooldownMs: 800,
    blastRadiusTiles: 2,
    damage: 100,
    description: 'Concealed explosive trap. Explodes when an opponent steps near.',
  },
  nuke: {
    id: 'nuke',
    name: 'Nuclear Warhead',
    category: 'explosives',
    kind: 'explosive',
    cost: 1200,
    maxActive: 1,
    fuseMs: 3500,
    blastRadiusTiles: 8,
    damage: 200,
    penetration: 3,
    description: 'Apocalyptic mega-bomb! Obliterates a massive 8-tile zone.',
  },

  // Weapons & Projectiles
  rocket: {
    id: 'rocket',
    name: 'Mini-Rocket',
    category: 'weapons',
    kind: 'projectile',
    cost: 180,
    damage: 85,
    blastRadiusTiles: 2,
    description: 'Fires a straight-line rocket across tunnels until impact.',
  },
  flamethrower: {
    id: 'flamethrower',
    name: 'Flamethrower',
    category: 'weapons',
    kind: 'special',
    cost: 350,
    damage: 40,
    description: 'Shoots a scorching jet of fire that incinerates dirt and foes.',
  },

  // Mining Gear & Upgrades
  pickaxe_1: {
    id: 'pickaxe_1',
    name: 'Standard Pickaxe',
    category: 'gear',
    kind: 'tool',
    cost: 0,
    digMultiplier: 1.0,
    description: 'Starter pickaxe.',
  },
  pickaxe_2: {
    id: 'pickaxe_2',
    name: 'Hardened Pickaxe II',
    category: 'gear',
    kind: 'tool',
    cost: 500,
    maxOwned: 1,
    digMultiplier: 1.5,
    description: '1.5x faster tunneling speed. Permanent match upgrade.',
  },
  power_drill: {
    id: 'power_drill',
    name: 'Power Drill',
    category: 'gear',
    kind: 'tool',
    cost: 1000,
    maxOwned: 1,
    digMultiplier: 2.5,
    description: 'Pneumatic mining drill. 2.5x super-speed excavation!',
  },
  kevlar_armor: {
    id: 'kevlar_armor',
    name: 'Kevlar Armor',
    category: 'gear',
    kind: 'passive',
    cost: 450,
    maxOwned: 1,
    damageReduction: 0.40,
    description: 'Reinforced vest. Reduces all damage taken by 40%.',
  },
  teleport: {
    id: 'teleport',
    name: 'Teleporter',
    category: 'gear',
    kind: 'consumable',
    cost: 250,
    description: 'Emergency escape device. Warps you to a safe cavern.',
  },
  med_kit: {
    id: 'med_kit',
    name: 'First Aid Kit',
    category: 'gear',
    kind: 'consumable',
    cost: 150,
    heal: 50,
    description: 'Restores 50 HP immediately.',
  },
} as const satisfies Record<string, EquipmentDef>;

export type EquipmentId = keyof typeof EQUIPMENT;
