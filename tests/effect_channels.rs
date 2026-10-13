//! Sound effects play on the original's mixer channels: explosions share channels 8..=10 and cut off the sound
//! closest to its end when all three are busy; other effects keep a fixed channel.
//!
//! Runs on SDL's `dummy` audio driver (no sound device) with stand-in samples, so no original files are needed.

use mb_reloaded::effects::{SoundEffect, SoundEffects};
use mb_reloaded::world::position::Cursor;
use sdl2::mixer::{Channel, AUDIO_S16LSB};
use std::time::Duration;

const FILES: [&str; 12] = [
  "KILI.VOC",
  "PICAXE.VOC",
  "EXPLOS1.VOC",
  "EXPLOS2.VOC",
  "EXPLOS3.VOC",
  "EXPLOS4.VOC",
  "EXPLOS5.VOC",
  "AARGH.VOC",
  "KARJAISU.VOC",
  "PIKKUPOM.VOC",
  "URETHAN.VOC",
  "APPLAUSE.VOC",
];

fn playing() -> Vec<i32> {
  (0..sdl2::mixer::allocate_channels(-1))
    .filter(|&channel| Channel(channel).is_playing())
    .collect()
}

#[test]
fn effects_use_the_original_channels() {
  std::env::set_var("SDL_AUDIODRIVER", "dummy");
  let sdl = sdl2::init().unwrap();
  let _audio = sdl.audio().unwrap();
  sdl2::mixer::open_audio(44100, AUDIO_S16LSB, 2, 1024).unwrap();
  sdl2::mixer::allocate_channels(i32::from(mb_core::sound::SOUND_CHANNELS));

  // Each stand-in sample lasts a few seconds at 11 kHz: long enough to stay busy while the test looks.
  let dir = std::env::temp_dir().join(format!("mb-effect-channels-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  for file in FILES {
    std::fs::write(dir.join(file), vec![140u8; 40_000]).unwrap();
  }
  let effects = SoundEffects::new(&dir).unwrap();
  let at = Cursor::new(10, 10);

  effects.play(SoundEffect::Explos1, 11000, at).unwrap();
  effects.play(SoundEffect::Explos2, 11000, at).unwrap();
  effects.play(SoundEffect::Pikkupom, 11000, at).unwrap();
  assert_eq!(playing(), vec![8, 9, 10]);

  // All three busy: the fourth explosion replaces one of them instead of taking another channel.
  effects.play(SoundEffect::Explos3, 11000, at).unwrap();
  assert_eq!(playing(), vec![8, 9, 10]);

  effects.play(SoundEffect::Kili, 11000, at).unwrap();
  effects.play(SoundEffect::Picaxe, 11000, at).unwrap();
  effects.play(SoundEffect::Aargh, 11000, at).unwrap();
  assert_eq!(playing(), vec![6, 7, 8, 9, 10]);

  for channel in 0..11 {
    Channel(channel).halt();
  }
  std::thread::sleep(Duration::from_millis(50));
  drop(effects);
  let _ = std::fs::remove_dir_all(&dir);
}
