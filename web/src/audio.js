import { ChiptuneJsPlayer } from '../vendor/chiptune3/chiptune3.js';
import { SFX_FILES, BGM_FILE, decodeVoc } from './assets.js';

const BGM_VOLUME = 0.45;

export class AudioManager {
  constructor() {
    this.ctx = null;
    // Indexed like SFX_FILES / the wasm module's AudioEvent.effect_id
    this.buffers = [];
    this.bgmData = null;
    this.bgm = null;
    this.bgmReady = false;
    this.bgmWanted = false;
    this.bgmPlaying = false;
    this.muted = false;
  }

  ensureContext() {
    if (!this.ctx) {
      const AudioContext = window.AudioContext || window.webkitAudioContext;
      this.ctx = new AudioContext();
    }
    if (this.ctx.state === 'suspended') {
      this.ctx.resume();
    }
  }

  /**
   * Builds the sound effects from the original .VOC files and keeps the original tracker module
   * for the background music.
   * @param {Map<string, Uint8Array>} files original game files, see assets.js
   */
  preload(files) {
    this.ensureContext();
    SFX_FILES.forEach((name, id) => {
      try {
        const { pcm, sampleRate } = decodeVoc(files.get(name));
        const buffer = this.ctx.createBuffer(1, pcm.length, sampleRate);
        const samples = buffer.getChannelData(0);
        for (let i = 0; i < pcm.length; i++) {
          samples[i] = (pcm[i] - 128) / 128; // 8-bit unsigned PCM
        }
        this.buffers[id] = buffer;
      } catch (e) {
        console.warn('Failed to load sound:', name, e);
      }
    });
    this.bgmData = files.get(BGM_FILE) || null;
  }

  playSfx(id, freq = 11000, pan = 0.0) {
    if (this.muted || !this.ctx) return;
    const buffer = this.buffers[id];
    if (!buffer) return;

    try {
      const source = this.ctx.createBufferSource();
      source.buffer = buffer;

      // Pitch/frequency scaling
      if (freq > 0 && freq !== 11000) {
        source.playbackRate.value = Math.max(0.25, Math.min(3.0, freq / 11000.0));
      }

      // Panner
      let lastNode = source;
      if (typeof this.ctx.createStereoPanner === 'function') {
        const panner = this.ctx.createStereoPanner();
        panner.pan.value = Math.max(-1.0, Math.min(1.0, pan));
        source.connect(panner);
        lastNode = panner;
      }

      const gain = this.ctx.createGain();
      gain.gain.value = 0.8;
      lastNode.connect(gain);
      gain.connect(this.ctx.destination);

      source.start(0);
    } catch (e) {
      console.warn('Audio play error:', e);
    }
  }

  startBgm() {
    if (!this.bgmData || !this.ctx || this.bgmPlaying) return;
    try {
      if (!this.bgm) {
        // libopenmpt (via chiptune3) plays the original Scream Tracker 3 module
        const player = new ChiptuneJsPlayer({ context: this.ctx });
        player.gain.connect(this.ctx.destination);
        player.setVol(this.muted ? 0 : BGM_VOLUME);
        player.onInitialized(() => {
          this.bgmReady = true;
          this.playBgm();
        });
        player.onError((e) => console.warn('BGM error:', e));
        this.bgm = player;
      }
      this.bgmWanted = true;
      this.playBgm();
    } catch (e) {
      console.warn('BGM unavailable:', e);
    }
  }

  playBgm() {
    if (!this.bgmWanted || !this.bgmReady || this.bgmPlaying) return;
    this.bgm.play(this.bgmData.slice().buffer); // loops until stopped
    this.bgmPlaying = true;
  }

  stopBgm() {
    this.bgmWanted = false;
    if (this.bgm && this.bgmPlaying) {
      this.bgm.stop();
      this.bgmPlaying = false;
    }
  }

  toggleMute() {
    this.muted = !this.muted;
    if (this.bgm) {
      this.bgm.setVol(this.muted ? 0 : BGM_VOLUME);
    }
    return this.muted;
  }
}
