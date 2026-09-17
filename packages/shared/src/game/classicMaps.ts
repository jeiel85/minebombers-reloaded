import classicMapsJson from './classicMapsData.json';
import type { GeneratedMap, GridPoint, GeneratedTreasure } from './mapGenerator';
import type { TileKind } from '../protocol';
import { MAP_HEIGHT, MAP_WIDTH } from './constants';

export interface ClassicMapData {
  name: string;
  width: number;
  height: number;
  tiles: TileKind[];
  spawns: GridPoint[];
  treasures: GeneratedTreasure[];
  monsters: Array<{ tileX: number; tileY: number; kind: 'slime' | 'bat' }>;
}

export const CLASSIC_MAPS: Record<string, ClassicMapData> = classicMapsJson as unknown as Record<string, ClassicMapData>;

export const CLASSIC_MAP_NAMES: string[] = Object.keys(CLASSIC_MAPS).sort();

export function getClassicMap(name: string, playerCount = 4): GeneratedMap | null {
  const upper = name.toUpperCase();
  const data = CLASSIC_MAPS[upper];
  if (!data) return null;

  return {
    generator: 'classic-mine-v1',
    seed: 0,
    width: data.width || MAP_WIDTH,
    height: data.height || MAP_HEIGHT,
    tiles: [...data.tiles],
    spawns: data.spawns.slice(0, playerCount).map((p) => ({ ...p })),
    treasures: data.treasures.map((t) => ({ ...t })),
  };
}

export function isValidClassicMap(name: string): boolean {
  return Boolean(CLASSIC_MAPS[name.toUpperCase()]);
}
