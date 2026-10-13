#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundEffect {
  Kili,
  Picaxe,
  Explos1,
  Explos2,
  Explos3,
  Explos4,
  Explos5,
  Aargh,
  Karjaisu,
  Pikkupom,
  Urethan,
  Applause,
}

/// Number of mixer channels the original's Sound Blaster driver mixes (0..=10).
pub const SOUND_CHANNELS: u8 = 11;

/// Mixer channels a sound effect may play on, as the original 3.11 assigns them for Sound Blaster cards without
/// "For SLOW computer" (`MB.EXE` segment 3 `0x52e3`, channel choice in segment 2 `0x317f`).
///
/// The Gravis Ultrasound path differs: explosions share voices 9..=11 and replace the last one when all three are
/// busy, while every other sound passes a channel count of 0 and so plays on an uninitialized voice number (segment 2
/// `0x1733`). That cannot be reproduced, so the Sound Blaster assignment is the reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundChannels {
  /// Always this channel; a new sound cuts off whatever plays there.
  Fixed(u8),
  /// Any of `count` channels starting at `first`, see [`pick_pool_channel`].
  Pool { first: u8, count: u8 },
}

impl SoundEffect {
  pub fn channels(self) -> SoundChannels {
    match self {
      SoundEffect::Explos1
      | SoundEffect::Explos2
      | SoundEffect::Explos3
      | SoundEffect::Explos4
      | SoundEffect::Explos5
      | SoundEffect::Pikkupom
      | SoundEffect::Urethan => SoundChannels::Pool { first: 8, count: 3 },
      SoundEffect::Applause => SoundChannels::Fixed(10),
      SoundEffect::Kili | SoundEffect::Picaxe => SoundChannels::Fixed(7),
      SoundEffect::Karjaisu | SoundEffect::Aargh => SoundChannels::Fixed(6),
    }
  }
}

/// Picks a channel from `count` channels starting at `first`, as the original does: the first free one, otherwise the
/// one with the fewest source samples left to play (the first of equals), whose sound is cut off.
///
/// `remaining` gives the number of source samples a channel has left; 0 means the channel is free.
pub fn pick_pool_channel(first: u8, count: u8, remaining: impl Fn(u8) -> u32) -> u8 {
  let channels = first..first + count;
  if let Some(free) = channels.clone().find(|&channel| remaining(channel) == 0) {
    return free;
  }
  // `min_by_key` keeps the first of equal elements.
  channels.min_by_key(|&channel| remaining(channel)).unwrap_or(first)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn explosions_share_three_channels_and_the_rest_have_fixed_ones() {
    for effect in [
      SoundEffect::Explos1,
      SoundEffect::Explos2,
      SoundEffect::Explos3,
      SoundEffect::Explos4,
      SoundEffect::Explos5,
      SoundEffect::Pikkupom,
      SoundEffect::Urethan,
    ] {
      assert_eq!(
        effect.channels(),
        SoundChannels::Pool { first: 8, count: 3 },
        "{:?}",
        effect
      );
    }
    assert_eq!(SoundEffect::Applause.channels(), SoundChannels::Fixed(10));
    assert_eq!(SoundEffect::Kili.channels(), SoundChannels::Fixed(7));
    assert_eq!(SoundEffect::Picaxe.channels(), SoundChannels::Fixed(7));
    assert_eq!(SoundEffect::Karjaisu.channels(), SoundChannels::Fixed(6));
    assert_eq!(SoundEffect::Aargh.channels(), SoundChannels::Fixed(6));
  }

  #[test]
  fn every_channel_fits_the_mixer() {
    for effect in [
      SoundEffect::Kili,
      SoundEffect::Picaxe,
      SoundEffect::Explos1,
      SoundEffect::Aargh,
      SoundEffect::Karjaisu,
      SoundEffect::Pikkupom,
      SoundEffect::Urethan,
      SoundEffect::Applause,
    ] {
      let last = match effect.channels() {
        SoundChannels::Fixed(channel) => channel,
        SoundChannels::Pool { first, count } => first + count - 1,
      };
      assert!(last < SOUND_CHANNELS, "{:?}", effect);
    }
  }

  #[test]
  fn pool_takes_the_first_free_channel() {
    let left = [0, 0, 0, 0, 0, 0, 0, 0, 500, 0, 0];
    assert_eq!(pick_pool_channel(8, 3, |c| left[c as usize]), 9);
    let left = [0; 11];
    assert_eq!(pick_pool_channel(8, 3, |c| left[c as usize]), 8);
  }

  #[test]
  fn full_pool_cuts_the_sound_closest_to_its_end() {
    let left = [0, 0, 0, 0, 0, 0, 0, 0, 900, 120, 400];
    assert_eq!(pick_pool_channel(8, 3, |c| left[c as usize]), 9);
  }

  #[test]
  fn full_pool_with_equal_sounds_cuts_the_first() {
    let left = [0, 0, 0, 0, 0, 0, 0, 0, 300, 300, 300];
    assert_eq!(pick_pool_channel(8, 3, |c| left[c as usize]), 8);
  }
}
