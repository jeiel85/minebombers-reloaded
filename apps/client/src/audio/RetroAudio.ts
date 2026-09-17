/**
 * RetroAudio: Procedural 8-bit sound synthesizer using Web Audio API.
 * Recreates authentic Sound Blaster / DOS era sound effects with zero external asset dependencies.
 */
export class RetroAudio {
  private static ctx: AudioContext | null = null;
  private static enabled = true;
  private static musicEnabled = true;
  private static soundBuffers = new Map<string, AudioBuffer>();
  private static loadingBuffers = false;
  private static bgmAudio: HTMLAudioElement | null = null;
  private static currentTrack: string | null = null;

  public static init(): void {
    const ctx = this.getContext();
    if (!ctx || this.loadingBuffers || this.soundBuffers.size > 0) return;
    this.loadingBuffers = true;

    const baseUrl = (typeof import.meta !== 'undefined' && import.meta.env && import.meta.env.BASE_URL) || './';
    const cleanBase = baseUrl.endsWith('/') ? baseUrl : baseUrl + '/';

    const files: Record<string, string> = {
      picaxe: `${cleanBase}assets/audio/sfx_picaxe.wav`,
      aargh: `${cleanBase}assets/audio/sfx_aargh.wav`,
      explos1: `${cleanBase}assets/audio/sfx_explos1.wav`,
      explos2: `${cleanBase}assets/audio/sfx_explos2.wav`,
      explos3: `${cleanBase}assets/audio/sfx_explos3.wav`,
      explos4: `${cleanBase}assets/audio/sfx_explos4.wav`,
      explos5: `${cleanBase}assets/audio/sfx_explos5.wav`,
      applause: `${cleanBase}assets/audio/sfx_applause.wav`,
      pikkupom: `${cleanBase}assets/audio/sfx_pikkupom.wav`,
      karjaisu: `${cleanBase}assets/audio/sfx_karjaisu.wav`,
      kili: `${cleanBase}assets/audio/sfx_kili.wav`,
      urethan: `${cleanBase}assets/audio/sfx_urethan.wav`,
    };

    for (const [key, url] of Object.entries(files)) {
      fetch(url)
        .then((res) => (res.ok ? res.arrayBuffer() : null))
        .then((buf) => {
          if (buf && this.ctx) {
            this.ctx.decodeAudioData(buf, (decoded) => {
              this.soundBuffers.set(key, decoded);
            });
          }
        })
        .catch(() => {});
    }
  }

  public static playBuffer(name: string, volume = 0.5): boolean {
    const ctx = this.getContext();
    if (!ctx) return false;
    const buf = this.soundBuffers.get(name);
    if (!buf) return false;

    try {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      const gain = ctx.createGain();
      gain.gain.setValueAtTime(volume, ctx.currentTime);
      src.connect(gain);
      gain.connect(ctx.destination);
      src.start();
      return true;
    } catch {
      return false;
    }
  }

  private static getContext(): AudioContext | null {
    if (!this.enabled) return null;
    if (!this.ctx && typeof window !== 'undefined') {
      const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      if (AudioCtx) {
        this.ctx = new AudioCtx();
        this.init();
      }
    }
    if (this.ctx && this.ctx.state === 'suspended') {
      this.ctx.resume().catch(() => {});
    }
    return this.ctx;
  }

  public static setEnabled(val: boolean): void {
    this.enabled = val;
  }

  public static isEnabled(): boolean {
    return this.enabled;
  }

  public static setMusicEnabled(val: boolean): void {
    this.musicEnabled = val;
    if (!val) {
      this.stopBGM();
    } else if (this.currentTrack) {
      this.playBGM(this.currentTrack as 'huippe' | 'oeku');
    }
  }

  public static isMusicEnabled(): boolean {
    return this.musicEnabled;
  }

  /**
   * Stream authentic 1995 Scream Tracker 3 BGM (Huipentuja / Oeku)
   */
  public static playBGM(track: 'huippe' | 'oeku', volume = 0.35): void {
    if (!this.musicEnabled || typeof window === 'undefined') return;
    if (this.currentTrack === track && this.bgmAudio && !this.bgmAudio.paused) return;
    this.stopBGM();

    try {
      const baseUrl = (typeof import.meta !== 'undefined' && import.meta.env && import.meta.env.BASE_URL) || './';
      const cleanBase = baseUrl.endsWith('/') ? baseUrl : baseUrl + '/';
      const audio = new Audio(`${cleanBase}assets/audio/bgm_${track}.mp3`);
      audio.loop = true;
      audio.volume = volume;
      audio.play().catch(() => {});
      this.bgmAudio = audio;
      this.currentTrack = track;
    } catch {}
  }

  public static stopBGM(): void {
    if (this.bgmAudio) {
      this.bgmAudio.pause();
      this.bgmAudio.currentTime = 0;
      this.bgmAudio = null;
    }
  }

  /**
   * Excavating/digging soil crunch sound (original Sound Blaster sample with procedural fallback)
   */
  public static playDig(): void {
    if (this.playBuffer('picaxe', 0.6)) return;

    const ctx = this.getContext();
    if (!ctx) return;

    const bufferSize = ctx.sampleRate * 0.08;
    const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < bufferSize; i++) {
      data[i] = (Math.random() * 2 - 1) * Math.exp(-i / (bufferSize * 0.4));
    }

    const noise = ctx.createBufferSource();
    noise.buffer = buffer;

    const filter = ctx.createBiquadFilter();
    filter.type = 'bandpass';
    filter.frequency.setValueAtTime(380, ctx.currentTime);
    filter.Q.setValueAtTime(3.0, ctx.currentTime);

    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.18, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.08);

    noise.connect(filter);
    filter.connect(gain);
    gain.connect(ctx.destination);
    noise.start();
  }

  /**
   * Explosions (small, medium, heavy, nuke) - Original Sound Blaster samples EXPLOS1..5 with procedural rumble
   */
  public static playExplosion(intensity: 'small' | 'medium' | 'heavy' | 'nuke' = 'small'): void {
    const ctx = this.getContext();
    if (!ctx) return;

    let sampleKey = 'explos1';
    let vol = 0.5;
    if (intensity === 'nuke') {
      sampleKey = 'explos5';
      vol = 0.95;
    } else if (intensity === 'heavy') {
      const samples = ['explos3', 'explos4', 'explos5'];
      sampleKey = samples[Math.floor(Math.random() * samples.length)]!;
      vol = 0.75;
    } else if (intensity === 'medium') {
      const samples = ['explos2', 'explos3', 'explos4'];
      sampleKey = samples[Math.floor(Math.random() * samples.length)]!;
      vol = 0.6;
    } else {
      const samples = ['explos1', 'explos2'];
      sampleKey = samples[Math.floor(Math.random() * samples.length)]!;
      vol = 0.45;
    }

    const playedSample = this.playBuffer(sampleKey, vol);

    // If heavy or nuke, layer low-frequency sub-bass oscillator
    if (intensity === 'heavy' || intensity === 'nuke' || !playedSample) {
      const duration = intensity === 'nuke' ? 2.5 : intensity === 'heavy' ? 0.9 : intensity === 'medium' ? 0.6 : 0.4;
      const baseFreq = intensity === 'nuke' ? 45 : intensity === 'heavy' ? 60 : intensity === 'medium' ? 80 : 110;
      const masterVol = intensity === 'nuke' ? 0.45 : intensity === 'heavy' ? 0.35 : 0.25;

      // Low frequency thump
      const osc = ctx.createOscillator();
      osc.type = 'triangle';
      osc.frequency.setValueAtTime(baseFreq * 2.5, ctx.currentTime);
      osc.frequency.exponentialRampToValueAtTime(15, ctx.currentTime + duration);

      const oscGain = ctx.createGain();
      oscGain.gain.setValueAtTime(masterVol, ctx.currentTime);
      oscGain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + duration);

      osc.connect(oscGain);
      oscGain.connect(ctx.destination);
      osc.start();
      osc.stop(ctx.currentTime + duration);

      if (!playedSample) {
        // Fallback white noise roar if buffer not ready
        const bufferSize = ctx.sampleRate * duration;
        const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
        const data = buffer.getChannelData(0);
        for (let i = 0; i < bufferSize; i++) {
          data[i] = Math.random() * 2 - 1;
        }

        const noise = ctx.createBufferSource();
        noise.buffer = buffer;

        const filter = ctx.createBiquadFilter();
        filter.type = 'lowpass';
        filter.frequency.setValueAtTime(intensity === 'nuke' ? 1200 : 800, ctx.currentTime);
        filter.frequency.exponentialRampToValueAtTime(60, ctx.currentTime + duration);

        const noiseGain = ctx.createGain();
        noiseGain.gain.setValueAtTime(masterVol * 1.2, ctx.currentTime);
        noiseGain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + duration);

        noise.connect(filter);
        filter.connect(noiseGain);
        noiseGain.connect(ctx.destination);
        noise.start();
      }
    }
  }

  /**
   * Rocket fire whoosh
   */
  public static playRocket(): void {
    const ctx = this.getContext();
    if (!ctx) return;

    const osc = ctx.createOscillator();
    osc.type = 'sawtooth';
    osc.frequency.setValueAtTime(180, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(620, ctx.currentTime + 0.18);

    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.15, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.22);

    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.22);
  }

  /**
   * Flamethrower whoosh
   */
  public static playFlame(): void {
    const ctx = this.getContext();
    if (!ctx) return;

    const bufferSize = ctx.sampleRate * 0.25;
    const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < bufferSize; i++) {
      data[i] = Math.random() * 2 - 1;
    }

    const noise = ctx.createBufferSource();
    noise.buffer = buffer;

    const filter = ctx.createBiquadFilter();
    filter.type = 'bandpass';
    filter.frequency.setValueAtTime(450, ctx.currentTime);
    filter.Q.setValueAtTime(1.5, ctx.currentTime);

    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.2, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.25);

    noise.connect(filter);
    filter.connect(gain);
    gain.connect(ctx.destination);
    noise.start();
  }

  /**
   * Treasure / Pickup chime
   */
  public static playPickup(): void {
    const ctx = this.getContext();
    if (!ctx) return;

    const notes = [523.25, 659.25, 783.99, 1046.5]; // C5, E5, G5, C6
    notes.forEach((freq, idx) => {
      const osc = ctx.createOscillator();
      osc.type = 'sine';
      osc.frequency.setValueAtTime(freq, ctx.currentTime + idx * 0.04);

      const gain = ctx.createGain();
      gain.gain.setValueAtTime(0, ctx.currentTime + idx * 0.04);
      gain.gain.linearRampToValueAtTime(0.12, ctx.currentTime + idx * 0.04 + 0.01);
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + idx * 0.04 + 0.12);

      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.start(ctx.currentTime + idx * 0.04);
      osc.stop(ctx.currentTime + idx * 0.04 + 0.12);
    });
  }

  /**
   * Player or monster damaged grunt (original Sound Blaster AARGH sample with procedural fallback)
   */
  public static playHurt(): void {
    if (this.playBuffer('aargh', 0.65)) return;

    const ctx = this.getContext();
    if (!ctx) return;

    const osc = ctx.createOscillator();
    osc.type = 'square';
    osc.frequency.setValueAtTime(160, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(70, ctx.currentTime + 0.12);

    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.2, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.12);

    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.12);
  }

  /**
   * Bomb placement drop sound (original Sound Blaster PIKKUPOM sample)
   */
  public static playBombDrop(): void {
    if (this.playBuffer('pikkupom', 0.55)) return;

    const ctx = this.getContext();
    if (!ctx) return;

    const osc = ctx.createOscillator();
    osc.type = 'sine';
    osc.frequency.setValueAtTime(320, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(120, ctx.currentTime + 0.08);

    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.2, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.08);

    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.08);
  }

  /**
   * Round victory / Match win applause (original Sound Blaster APPLAUSE sample)
   */
  public static playVictory(): void {
    if (this.playBuffer('applause', 0.75)) return;
    this.playPickup();
  }

  /**
   * Shop purchase / Gold cash register (original Sound Blaster KILI sample)
   */
  public static playBuy(): void {
    if (this.playBuffer('kili', 0.65)) return;
    this.playPickup();
  }

  /**
   * Monster roar / Bat screech (original Sound Blaster KARJAISU sample)
   */
  public static playRoar(): void {
    if (this.playBuffer('karjaisu', 0.65)) return;
    this.playHurt();
  }

  /**
   * Teleport warp sound
   */
  public static playTeleport(): void {
    const ctx = this.getContext();
    if (!ctx) return;

    const osc = ctx.createOscillator();
    osc.type = 'triangle';
    osc.frequency.setValueAtTime(220, ctx.currentTime);
    osc.frequency.linearRampToValueAtTime(880, ctx.currentTime + 0.1);
    osc.frequency.exponentialRampToValueAtTime(110, ctx.currentTime + 0.25);

    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.18, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.25);

    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.25);
  }

  /**
   * Mine collapse alarm siren
   */
  public static playAlarm(): void {
    const ctx = this.getContext();
    if (!ctx) return;

    const osc = ctx.createOscillator();
    osc.type = 'sawtooth';
    osc.frequency.setValueAtTime(440, ctx.currentTime);
    osc.frequency.setValueAtTime(660, ctx.currentTime + 0.15);
    osc.frequency.setValueAtTime(440, ctx.currentTime + 0.3);

    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.15, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.45);

    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.45);
  }

  /**
   * UI Click
   */
  public static playClick(): void {
    const ctx = this.getContext();
    if (!ctx) return;

    const osc = ctx.createOscillator();
    osc.type = 'triangle';
    osc.frequency.setValueAtTime(800, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(400, ctx.currentTime + 0.04);

    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.1, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.04);

    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.04);
  }
}
