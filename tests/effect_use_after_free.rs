//! Regression test for a crash in `SDL2_mixer.dll` (`Mix_DoEffects`), which showed up as the game suddenly closing.
//!
//! An effect callback used to stop its own channel with `Mix_HaltChannel`. That frees the effect the mixer is running,
//! and the mixer reads it once more when the callback returns (`e = e->next`): a use-after-free on the audio thread.
//! What it does depends on what the allocator leaves in freed memory, so it only crashed now and then. Here SDL gets an
//! allocator that fills freed blocks with a poison pattern and never reuses them, so any read of freed memory fails
//! right away: the test process dies with an access violation (SIGSEGV) instead of passing.
//!
//! The test sits in the main package, not in `mb-sdl2-effects`, on purpose: cargo starts it from the repository root,
//! where Windows finds the bundled SDL2 DLLs. It is the only test in the file because `SDL_SetMemoryFunctions` is
//! process-wide and has to run before SDL allocates anything. The audio goes to SDL's `dummy` driver, which needs no
//! sound device and plays nothing.

use mb_sdl2_effects::play_sound_sample;
use sdl2::mixer::{Channel, AUDIO_S16LSB};
use sdl2::sys::size_t;
use std::alloc::{alloc, Layout};
use std::ffi::c_void;
use std::ptr::{copy_nonoverlapping, null_mut, write_bytes};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Room in front of every block for its size; a multiple of 16 keeps the blocks aligned.
const HEADER: usize = 16;
const POISON: u8 = 0xDD;

fn allocate(size: usize, zeroed: bool) -> *mut c_void {
  let Ok(layout) = Layout::from_size_align(size + HEADER, HEADER) else {
    return null_mut();
  };
  let block = unsafe { alloc(layout) };
  if block.is_null() {
    return null_mut();
  }
  unsafe {
    block.cast::<usize>().write(size);
    if zeroed {
      write_bytes(block.add(HEADER), 0, size);
    }
    block.add(HEADER).cast()
  }
}

unsafe extern "C" fn poison_malloc(size: size_t) -> *mut c_void {
  allocate(size as usize, false)
}

unsafe extern "C" fn poison_calloc(count: size_t, size: size_t) -> *mut c_void {
  match (count as usize).checked_mul(size as usize) {
    Some(total) => allocate(total, true),
    None => null_mut(),
  }
}

unsafe extern "C" fn poison_realloc(ptr: *mut c_void, size: size_t) -> *mut c_void {
  if ptr.is_null() {
    return allocate(size as usize, false);
  }
  let old_size = ptr.cast::<u8>().sub(HEADER).cast::<usize>().read();
  let new = allocate(size as usize, false);
  if !new.is_null() {
    copy_nonoverlapping(ptr.cast::<u8>(), new.cast::<u8>(), old_size.min(size as usize));
    poison_free(ptr);
  }
  new
}

unsafe extern "C" fn poison_free(ptr: *mut c_void) {
  if ptr.is_null() {
    return;
  }
  let size = ptr.cast::<u8>().sub(HEADER).cast::<usize>().read();
  write_bytes(ptr.cast::<u8>(), POISON, size);
  // The block is deliberately not given back: freed memory is never reused, so the poison stays in place.
}

#[test]
fn finished_sounds_do_not_touch_freed_effects() {
  let installed = unsafe {
    sdl2::sys::SDL_SetMemoryFunctions(
      Some(poison_malloc),
      Some(poison_calloc),
      Some(poison_realloc),
      Some(poison_free),
    )
  };
  assert_eq!(installed, 0, "could not install the poisoning allocator");
  std::env::set_var("SDL_AUDIODRIVER", "dummy");

  let sdl = sdl2::init().unwrap();
  let _audio = sdl.audio().unwrap();
  // The parameters the game opens the mixer with.
  sdl2::mixer::open_audio(44100, AUDIO_S16LSB, 2, 1024).unwrap();

  // About 36 ms of 8-bit unsigned audio at 11 kHz: over within two mixer buffers, so sounds keep ending.
  let sample: Arc<[u8]> = (0..400u32)
    .map(|i| 128u8.wrapping_add((i * 13 % 50) as u8 + 10))
    .collect::<Vec<_>>()
    .into();
  let stop = Instant::now() + Duration::from_secs(2);
  while Instant::now() < stop {
    play_sound_sample(Channel::all(), 11000, sample.clone(), 0.5).unwrap();
    std::thread::sleep(Duration::from_millis(2));
  }

  // Nobody stops the channels: the mixer has to end them by itself.
  let deadline = Instant::now() + Duration::from_secs(5);
  while (0..8).any(|channel| Channel(channel).is_playing()) && Instant::now() < deadline {
    std::thread::sleep(Duration::from_millis(10));
  }
  let busy: Vec<i32> = (0..8).filter(|&channel| Channel(channel).is_playing()).collect();
  assert!(
    busy.is_empty(),
    "channels {:?} are still playing after their sounds are over",
    busy
  );
}
