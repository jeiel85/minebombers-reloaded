//! Crate with lower-level functions to emulate sound effects of the original game.
//! `SDL_RegisterEffect` is not supported by Rust bindings in `sdl2` crate, so we use lower-level C API.
//! Due to `unsafe` use, this is extracted into a separate crate to keep main crate clean of unsafe.

use libc::{c_int, c_void};
use sdl2::audio::AudioFormatNum;
use sdl2::mixer::{AudioFormat, Channel, Chunk};
use std::sync::Arc;

const BUF_LEN: usize = 4096;
static mut BUF: [u8; BUF_LEN] = [0; BUF_LEN];

/// Used to make mixer to play something. We don't really use these values at all -- we generate
/// sound samples directly in the registered effect.
static mut PLACEHOLDER: sdl2_sys::mixer::Mix_Chunk = sdl2_sys::mixer::Mix_Chunk {
  allocated: 0,
  abuf: &raw mut BUF as *mut [u8] as *mut u8,
  alen: BUF_LEN as u32,
  volume: 128,
};

/// Extra passes of the placeholder chunk after the sound. The mixer may run a pass between `Mix_PlayChannel` and
/// `Mix_RegisterEffect`: nothing can be heard in it, but it uses up one of the passes. A little slack keeps the end of
/// a sound from being cut in that case. The extra passes are silent; at 44.1 kHz with 1024 frame buffers, two of them
/// keep the channel about 46 ms longer.
const EXTRA_PASSES: u64 = 2;

/// Play sound effect on a given channel with a given playback frequency located at `position`.
///
/// The mixer releases the channel by itself when the sound is over.
pub fn play_sound_sample(channel: Channel, frequency: i32, chunk: Arc<[u8]>, position: f32) -> Result<(), String> {
  if frequency <= 0 {
    return Err(format!("Invalid playback frequency: {frequency}"));
  }
  let (mixer_frequency, format, channels) = sdl2::mixer::query_spec()?;
  let loops = placeholder_loops(chunk.len(), frequency, mixer_frequency, channels, format);
  let effect = Box::new(SampleCallback {
    channels: channels as usize,
    chunk,
    play_frequency: frequency,
    mixer_frequency,
    target_sample_offset: 0,
    position,
  });

  let placeholder = Chunk {
    raw: &raw mut PLACEHOLDER as *mut _,
    owned: false,
  };
  // FIXME: maybe, stop other channel?
  let channel = match channel.play(&placeholder, loops) {
    Ok(channel) => channel,
    Err(_) => return Ok(()),
  };
  let user_ptr = Box::into_raw(effect);

  let Channel(chan) = channel;
  let ret = unsafe {
    sdl2_sys::mixer::Mix_RegisterEffect(
      chan,
      gen_pitch_callback(format),
      Some(pitch_done_cb),
      user_ptr as *mut _,
    )
  };
  if ret == -1 {
    // Need to free the memory
    unsafe {
      let _ = Box::from_raw(user_ptr);
    }
    Err(sdl2::get_error())
  } else {
    Ok(())
  }
}

/// Number of repeats (the `loops` argument of `Mix_PlayChannel`, which counts the plays after the first one) that keep
/// the channel busy until the whole sample has been generated. `play_frequency` must be positive.
///
/// The effect writes the sound into the mixer's buffer while the mixer "plays" the silent placeholder chunk, so the
/// channel has to last as long as the sound. The mixer ends a channel by itself once the placeholder chunk has been
/// played the requested number of times. Ending it from inside the effect (`Mix_HaltChannel`) is not an option: the
/// mixer is walking the channel's list of effects at that moment and halting frees the list, so the mixer reads freed
/// memory as soon as the effect returns.
fn placeholder_loops(
  sample_len: usize,
  play_frequency: i32,
  mixer_frequency: i32,
  channels: i32,
  format: AudioFormat,
) -> i32 {
  let bytes_per_frame = (usize::from(format & 0xFF) / 8).max(1) * channels.max(1) as usize;
  let frames_per_pass = (BUF_LEN / bytes_per_frame).max(1) as u64;
  // Frames of output needed to play all source samples: ceil(sample_len * mixer_frequency / play_frequency).
  let frames = (sample_len as u64 * mixer_frequency as u64).div_ceil(play_frequency as u64);
  let passes = frames.div_ceil(frames_per_pass) + EXTRA_PASSES;
  (passes - 1).min(i32::MAX as u64) as i32
}

fn gen_pitch_callback(format: sdl2::mixer::AudioFormat) -> sdl2_sys::mixer::Mix_EffectFunc_t {
  let func = match format {
    sdl2::mixer::AUDIO_U8 => pitch_effect_cb_template::<u8>,
    sdl2::mixer::AUDIO_S8 => pitch_effect_cb_template::<i8>,
    sdl2::mixer::AUDIO_U16LSB => pitch_effect_cb_template::<u16>,
    sdl2::mixer::AUDIO_S16LSB => pitch_effect_cb_template::<i16>,
    sdl2::mixer::AUDIO_S32LSB => pitch_effect_cb_template::<i32>,
    sdl2::mixer::AUDIO_F32LSB => pitch_effect_cb_template::<f32>,

    // Need some types that will do conversion from MSB=>LBS or what?
    sdl2::mixer::AUDIO_U16MSB | sdl2::mixer::AUDIO_S16MSB | sdl2::mixer::AUDIO_S32MSB | sdl2::mixer::AUDIO_F32MSB => {
      unimplemented!()
    }
    _other => unreachable!(),
  };
  Some(func)
}

struct SampleCallback {
  /// Sample we want to play (single channel, unsigned, 8-bit).
  chunk: Arc<[u8]>,
  /// Frequency we want to play the sample
  play_frequency: i32,
  /// Horizontal pozition: 0.0 is the leftmost, 1.0 is the rightmost
  position: f32,
  /// Amount of channels current mixer has
  channels: usize,
  /// Frequency of the mixer we are targeting
  mixer_frequency: i32,
  /// Sample index (in the output format; basically, amount of samples we have generated so far).
  target_sample_offset: usize,
}

impl SampleCallback {
  /// Fills `stream` with the next part of the sound. Once the sound is over, the output is silence.
  fn generate_samples<T: AudioFormatNum + IntoSample>(&mut self, stream: &mut [T]) {
    let samples = stream.len() / self.channels;
    for sample in 0..samples {
      let output = &mut stream[(sample * self.channels)..][..self.channels];

      let target_sample = self.target_sample_offset + sample;
      let source_pos = (target_sample as f32) * (self.play_frequency as f32) / (self.mixer_frequency as f32);
      // round to floor
      let index = source_pos as usize;

      // Have source samples to interpolate
      if index < self.chunk.len() {
        let first = self.chunk[index];
        let second = self.chunk.get(index + 1).copied().unwrap_or(first);

        let fract = source_pos.fract();
        let first = f32::from(first.wrapping_sub(u8::SILENCE) as i8) / 256.0;
        let second = f32::from(second.wrapping_sub(u8::SILENCE) as i8) / 256.0;
        let sample = first * fract + second * (1.0 - fract);
        // Clamp the output
        let sample = if sample < -0.5 {
          -0.5
        } else if sample > 0.5 {
          0.5
        } else {
          sample
        };
        if self.channels == 1 {
          output[0] = IntoSample::from_f32(sample);
        } else {
          output[0] = IntoSample::from_f32(sample * (1.0 - self.position));
          output[1] = IntoSample::from_f32(sample * self.position);
        }
      } else {
        // We are done playing! Fill the rest with the silence. The mixer ends the channel by itself.
        for item in &mut stream[sample * self.channels..] {
          *item = T::SILENCE;
        }
        break;
      }
    }
    // Keep counting after the end of the sound: the effect is still called until the mixer ends the channel, and
    // it must not start over from an earlier position.
    self.target_sample_offset += samples;
  }
}

/// Effect callback, called by the mixer on its audio thread while it walks the channel's list of effects.
///
/// Do not call into SDL_mixer from here (`Mix_HaltChannel` and the like): stopping the channel frees the list of
/// effects the mixer is walking, see `placeholder_loops`.
extern "C" fn pitch_effect_cb_template<T: AudioFormatNum + IntoSample>(
  _chan: c_int,
  stream: *mut c_void,
  len: c_int,
  udata: *mut c_void,
) {
  // Sanity check
  if udata.is_null() {
    return;
  }

  let len = len as usize;
  let stream = unsafe { std::slice::from_raw_parts_mut(stream as *mut T, len / std::mem::size_of::<T>()) };
  let effect = unsafe { &mut *(udata as *mut SampleCallback) };
  effect.generate_samples(stream);
}

extern "C" fn pitch_done_cb(_chan: c_int, udata: *mut c_void) {
  // Sanity check
  if udata.is_null() {
    return;
  }
  let udata: *mut SampleCallback = udata as *mut _;
  unsafe {
    // Drop so we free all the memory we have allocated
    let _ = Box::from_raw(udata);
  }
}

/// Convert floating point in the range of the (-1.0f, 1.0f) to target sample type. 0.0f is the silence.
pub(crate) trait IntoSample: Copy {
  fn from_f32(sample: f32) -> Self;
}

impl IntoSample for u8 {
  fn from_f32(sample: f32) -> Self {
    (i8::from_f32(sample) as u8).wrapping_add(u8::SILENCE)
  }
}

impl IntoSample for i8 {
  fn from_f32(sample: f32) -> Self {
    (sample * 8.0_f32.exp2()) as i8
  }
}

impl IntoSample for u16 {
  fn from_f32(sample: f32) -> Self {
    (i16::from_f32(sample) as u16).wrapping_add(u16::SILENCE)
  }
}

impl IntoSample for i16 {
  fn from_f32(sample: f32) -> Self {
    (sample * 16.0_f32.exp2()) as i16
  }
}

impl IntoSample for i32 {
  fn from_f32(sample: f32) -> Self {
    (sample * 32.0_f32.exp2()) as i32
  }
}

impl IntoSample for f32 {
  fn from_f32(sample: f32) -> Self {
    sample
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use sdl2::mixer::{AUDIO_F32LSB, AUDIO_S16LSB, AUDIO_U8};

  /// Number of output frames that still hold part of the sample, worked out like `generate_samples` does.
  fn frames_with_data(sample_len: usize, play_frequency: i32, mixer_frequency: i32) -> usize {
    (0..)
      .find(|&frame| (frame as f32 * play_frequency as f32 / mixer_frequency as f32) as usize >= sample_len)
      .unwrap()
  }

  #[test]
  fn placeholder_lasts_as_long_as_the_sound_and_not_much_longer() {
    // (sample length, playback frequency, mixer frequency, mixer channels, mixer format)
    let cases = [
      (400, 11000, 44100, 2, AUDIO_S16LSB),
      (0, 11000, 44100, 2, AUDIO_S16LSB),
      (1, 11000, 44100, 2, AUDIO_S16LSB),
      (1024, 44100, 44100, 2, AUDIO_S16LSB),
      (1500, 5000, 44100, 2, AUDIO_S16LSB),
      (20000, 10300, 44100, 2, AUDIO_S16LSB),
      (5000, 22000, 22050, 1, AUDIO_U8),
      (3000, 12599, 48000, 2, AUDIO_F32LSB),
    ];
    for (len, play, mixer, channels, format) in cases {
      let frames_per_pass = BUF_LEN / (channels as usize * usize::from(format & 0xFF) / 8);
      let budget = (placeholder_loops(len, play, mixer, channels, format) as usize + 1) * frames_per_pass;
      let needed = frames_with_data(len, play, mixer);
      let case = format!("{len} samples at {play} Hz, mixer {mixer} Hz, {channels} channels, format {format:#x}");
      // The whole sound fits, with at least one pass to spare...
      assert!(budget >= needed + frames_per_pass, "the sound would be cut: {}", case);
      // ...and the channel is not kept busy for longer than the slack asks for.
      assert!(
        budget <= needed + (EXTRA_PASSES as usize + 1) * frames_per_pass,
        "the channel would be kept too long: {}",
        case
      );
    }
  }

  #[test]
  fn output_stays_silent_after_the_sound_is_over() {
    // 100 samples at 11 kHz make about 400 mixer frames at 44.1 kHz, which all fit in the first pass.
    let mut effect = SampleCallback {
      chunk: vec![200u8; 100].into(),
      play_frequency: 11000,
      position: 0.5,
      channels: 2,
      mixer_frequency: 44100,
      target_sample_offset: 0,
    };
    let mut pass = vec![i16::SILENCE; 1024 * 2];
    effect.generate_samples(&mut pass);
    assert!(
      pass[..400 * 2].iter().any(|&s| s != i16::SILENCE),
      "the sound is missing"
    );
    assert!(
      pass[402 * 2..].iter().all(|&s| s == i16::SILENCE),
      "the sound does not end"
    );

    // The mixer keeps calling the effect until it ends the channel; none of those calls may play anything.
    for _ in 0..3 {
      pass.fill(1234);
      effect.generate_samples(&mut pass);
      assert!(
        pass.iter().all(|&s| s == i16::SILENCE),
        "the sound started over after its end"
      );
    }
  }
}
