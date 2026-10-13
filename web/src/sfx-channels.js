// Mixer channels for the sound effects, the same rules as `SoundEffect::channels` and `pick_pool_channel` in
// crates/mb-core/src/sound.rs (the original's Sound Blaster driver): explosions share channels 8..=10, every other
// sound has a fixed channel, and a new sound cuts off whatever plays on the channel it gets.

import { SFX_FILES } from './assets.js';

export const SOUND_CHANNELS = 11;

const POOL = { first: 8, count: 3 };
const CHANNELS_BY_FILE = {
  'EXPLOS1.VOC': POOL,
  'EXPLOS2.VOC': POOL,
  'EXPLOS3.VOC': POOL,
  'EXPLOS4.VOC': POOL,
  'EXPLOS5.VOC': POOL,
  'PIKKUPOM.VOC': POOL,
  'URETHAN.VOC': POOL,
  'APPLAUSE.VOC': { first: 10, count: 1 },
  'KILI.VOC': { first: 7, count: 1 },
  'PICAXE.VOC': { first: 7, count: 1 },
  'KARJAISU.VOC': { first: 6, count: 1 },
  'AARGH.VOC': { first: 6, count: 1 },
};

/**
 * Channel for a sound effect: the first free one of its channels, otherwise the one with the fewest source samples
 * left (the first of equals).
 * @param {number} id effect id, an index into SFX_FILES
 * @param {(channel: number) => number} remaining source samples a channel has left; 0 when it is free
 */
export function pickChannel(id, remaining) {
  const { first, count } = CHANNELS_BY_FILE[SFX_FILES[id]];
  let best = first;
  for (let channel = first; channel < first + count; channel++) {
    const left = remaining(channel);
    if (left === 0) return channel;
    if (left < remaining(best)) best = channel;
  }
  return best;
}
