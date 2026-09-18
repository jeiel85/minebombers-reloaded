export class AudioManager {
  constructor() {
    this.ctx = null;
    this.buffers = {};
    this.bgm = null;
    this.muted = false;
    this.sfxNames = [
      'sfx_kili',     // 0
      'sfx_picaxe',   // 1
      'sfx_explos1',  // 2
      'sfx_explos2',  // 3
      'sfx_explos3',  // 4
      'sfx_explos4',  // 5
      'sfx_explos5',  // 6
      'sfx_aargh',    // 7
      'sfx_karjaisu', // 8
      'sfx_pikkupom', // 9
      'sfx_urethan',  // 10
      'sfx_applause', // 11
    ];
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

  async preload() {
    this.ensureContext();
    const promises = this.sfxNames.map(async (name) => {
      try {
        const res = await fetch(`./audio/${name}.wav`);
        if (!res.ok) return;
        const arrayBuf = await res.arrayBuffer();
        this.buffers[name] = await this.ctx.decodeAudioData(arrayBuf);
      } catch (e) {
        console.warn('Failed to load sound:', name, e);
      }
    });
    await Promise.all(promises);
  }

  playSfx(id, freq = 11000, pan = 0.0) {
    if (this.muted || !this.ctx) return;
    const name = this.sfxNames[id];
    if (!name || !this.buffers[name]) return;

    try {
      const source = this.ctx.createBufferSource();
      source.buffer = this.buffers[name];

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
    if (this.bgm) return;
    try {
      this.bgm = new Audio('./audio/bgm_oeku.mp3');
      this.bgm.loop = true;
      this.bgm.volume = 0.45;
      this.bgm.play().catch(() => {});
    } catch (e) {}
  }

  stopBgm() {
    if (this.bgm) {
      this.bgm.pause();
      this.bgm = null;
    }
  }

  toggleMute() {
    this.muted = !this.muted;
    if (this.bgm) {
      this.bgm.muted = this.muted;
    }
    return this.muted;
  }
}
