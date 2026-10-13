use crate::error::ApplicationError::SdlError;
use crate::world::map::MAP_COLS;
use crate::world::position::Cursor;
use mb_core::sound::{pick_pool_channel, SoundChannels, SOUND_CHANNELS};
use sdl2::mixer::Channel;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Failed to load sound sample from '{path}'")]
pub struct SampleLoadingFailed {
  path: PathBuf,
  source: anyhow::Error,
}

pub use mb_core::effects::SoundEffect;

/// VOC files are unsigned, eight bits, 1 channel, frequency defined at the playback time (typically 11000).
/// We use `Arc` here so we can give references to these samples to sound effects without worrying
/// about ownership.
#[derive(Clone)]
struct RawSample(Arc<[u8]>);

pub struct SoundEffects {
  kili: RawSample,
  picaxe: RawSample,
  explos1: RawSample,
  explos2: RawSample,
  explos3: RawSample,
  explos4: RawSample,
  explos5: RawSample,
  aargh: RawSample,
  karjaisu: RawSample,
  pikkupom: RawSample,
  urethan: RawSample,
  applause: RawSample,
  /// What was last started on each mixer channel, to tell how much of it is left.
  playing: RefCell<[Option<Playing>; SOUND_CHANNELS as usize]>,
}

#[derive(Clone, Copy)]
struct Playing {
  started: Instant,
  samples: usize,
  frequency: i32,
}

impl Playing {
  /// Source samples still to play; 0 once the sound is over.
  fn remaining(&self, now: Instant) -> u32 {
    let played = now.duration_since(self.started).as_secs_f64() * f64::from(self.frequency);
    (self.samples as f64 - played).clamp(0.0, f64::from(u32::MAX)).ceil() as u32
  }
}

impl SoundEffects {
  /// Initialize game sound effects given the game directory
  pub fn new(path: &Path) -> Result<Self, anyhow::Error> {
    Ok(SoundEffects {
      kili: load_sample(path.join("KILI.VOC"))?,
      picaxe: load_sample(path.join("PICAXE.VOC"))?,
      explos1: load_sample(path.join("EXPLOS1.VOC"))?,
      explos2: load_sample(path.join("EXPLOS2.VOC"))?,
      explos3: load_sample(path.join("EXPLOS3.VOC"))?,
      explos4: load_sample(path.join("EXPLOS4.VOC"))?,
      explos5: load_sample(path.join("EXPLOS5.VOC"))?,
      aargh: load_sample(path.join("AARGH.VOC"))?,
      karjaisu: load_sample(path.join("KARJAISU.VOC"))?,
      pikkupom: load_sample(path.join("PIKKUPOM.VOC"))?,
      urethan: load_sample(path.join("URETHAN.VOC"))?,
      applause: load_sample(path.join("APPLAUSE.VOC"))?,
      playing: RefCell::new([None; SOUND_CHANNELS as usize]),
    })
  }

  /// Play sound effect on the channel the original would use, cutting off what plays there.
  pub fn play(&self, effect: SoundEffect, frequency: i32, location: Cursor) -> Result<(), anyhow::Error> {
    let position = f32::from(location.col) / f32::from(MAP_COLS - 1);
    let channels = effect.channels();
    let effect = match effect {
      SoundEffect::Kili => &self.kili,
      SoundEffect::Picaxe => &self.picaxe,
      SoundEffect::Explos1 => &self.explos1,
      SoundEffect::Explos2 => &self.explos2,
      SoundEffect::Explos3 => &self.explos3,
      SoundEffect::Explos4 => &self.explos4,
      SoundEffect::Explos5 => &self.explos5,
      SoundEffect::Aargh => &self.aargh,
      SoundEffect::Karjaisu => &self.karjaisu,
      SoundEffect::Pikkupom => &self.pikkupom,
      SoundEffect::Urethan => &self.urethan,
      SoundEffect::Applause => &self.applause,
    };
    let now = Instant::now();
    let mut playing = self.playing.borrow_mut();
    let channel = match channels {
      SoundChannels::Fixed(channel) => channel,
      SoundChannels::Pool { first, count } => pick_pool_channel(first, count, |channel| {
        playing[usize::from(channel)].map_or(0, |sound| sound.remaining(now))
      }),
    };
    // Halt from here, not from the effect callback: see `mb_sdl2_effects::play_sound_sample`.
    Channel(i32::from(channel)).halt();
    playing[usize::from(channel)] = Some(Playing {
      started: now,
      samples: effect.0.len(),
      frequency,
    });
    mb_sdl2_effects::play_sound_sample(Channel(i32::from(channel)), frequency, effect.0.clone(), position)
      .map_err(SdlError)?;
    Ok(())
  }
}

fn load_sample(path: PathBuf) -> Result<RawSample, SampleLoadingFailed> {
  let data = std::fs::read(&path).map_err(|source| SampleLoadingFailed {
    path,
    source: source.into(),
  })?;
  Ok(RawSample(data.into()))
}
