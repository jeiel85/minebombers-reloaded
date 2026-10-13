import test from 'node:test';
import assert from 'node:assert/strict';
import { SFX_FILES } from '../src/assets.js';
import { SOUND_CHANNELS, pickChannel } from '../src/sfx-channels.js';

const id = (file) => SFX_FILES.indexOf(file);
const left = (values) => (channel) => values[channel] ?? 0;

test('every effect has channels inside the mixer', () => {
  SFX_FILES.forEach((file, effect) => {
    const channel = pickChannel(effect, () => 0);
    assert.ok(channel >= 0 && channel < SOUND_CHANNELS, file);
  });
});

test('effects without a pool keep their fixed channel even when it is busy', () => {
  const busy = () => 500;
  assert.equal(pickChannel(id('APPLAUSE.VOC'), busy), 10);
  assert.equal(pickChannel(id('KILI.VOC'), busy), 7);
  assert.equal(pickChannel(id('PICAXE.VOC'), busy), 7);
  assert.equal(pickChannel(id('KARJAISU.VOC'), busy), 6);
  assert.equal(pickChannel(id('AARGH.VOC'), busy), 6);
});

test('explosions take the first free of channels 8 to 10', () => {
  for (const file of ['EXPLOS1.VOC', 'EXPLOS5.VOC', 'PIKKUPOM.VOC', 'URETHAN.VOC']) {
    assert.equal(pickChannel(id(file), left({})), 8, file);
    assert.equal(pickChannel(id(file), left({ 8: 500 })), 9, file);
  }
});

test('a full pool cuts the sound closest to its end, the first of equals', () => {
  assert.equal(pickChannel(id('EXPLOS2.VOC'), left({ 8: 900, 9: 120, 10: 400 })), 9);
  assert.equal(pickChannel(id('EXPLOS2.VOC'), left({ 8: 300, 9: 300, 10: 300 })), 8);
});
