use crate::error::ApplicationError::SdlError;
use crate::fonts::Font;
use crate::images::{TextureFormat, TexturePalette};
use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Scancode};
use sdl2::mixer::{Music, AUDIO_S16LSB};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Texture, TextureCreator, WindowCanvas};
use sdl2::surface::Surface;
use sdl2::video::WindowContext;
use sdl2::EventPump;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Application environment resources packaged into one structs. Provides helper functions used
/// across the whole application.
pub struct ApplicationContext<'canvas, 'textures> {
  pub game_dir: PathBuf,
  pub events: EventPump,
  pub canvas: &'canvas mut WindowCanvas,
  pub buffer: Texture<'textures>,
  pub texture_creator: &'textures TextureCreator<WindowContext>,
  pub config: crate::config::AppConfig,
  pub is_fullscreen: bool,
}

pub enum Animation {
  FadeUp,
  FadeDown,
}

/// Our representation of an input event. Most of the time, we only care about scancodes. However,
/// when we allow entering a text (player creation screen), we need to be able to represent input
/// text as well.
pub enum InputEvent {
  KeyPress(Scancode, Keycode),
  TextInput(String),
}

impl<'canvas, 'textures> ApplicationContext<'canvas, 'textures> {
  pub fn with_context(
    game_dir: PathBuf,
    cb: impl FnOnce(ApplicationContext) -> Result<(), anyhow::Error>,
  ) -> Result<(), anyhow::Error> {
    let sdl_context = sdl2::init().map_err(SdlError)?;
    let video = sdl_context.video().map_err(SdlError)?;

    let app_cfg = crate::config::AppConfig::load_or_create(&game_dir);
    let win_w = if app_cfg.display.width > 0 {
      app_cfg.display.width
    } else {
      SCREEN_WIDTH * app_cfg.display.scale.max(1)
    };
    let win_h = if app_cfg.display.height > 0 {
      app_cfg.display.height
    } else {
      SCREEN_HEIGHT * app_cfg.display.scale.max(1)
    };
    let mut window = video
      .window("Mine Bombers (Native Rust Engine)", win_w, win_h)
      .position_centered()
      .allow_highdpi()
      .resizable()
      .build()?;

    // Set application icon, we chop it off one of the game images
    let data = std::fs::read(game_dir.join("TITLEBE.SPY"))?;
    let mut spy = crate::images::decode_spy(SCREEN_WIDTH, SCREEN_HEIGHT, &data)?;
    let from = ((SCREEN_WIDTH * 305 + 265) * 3) as usize;
    let surface =
      Surface::from_data(&mut spy.image[from..], 96, 96, 3 * SCREEN_WIDTH, PixelFormatEnum::RGB24).map_err(SdlError)?;
    window.set_icon(&surface);
    window.set_grab(false);

    let is_fullscreen = match app_cfg.display.window_mode.to_lowercase().as_str() {
      "fullscreen" => {
        let _ = window.set_fullscreen(sdl2::video::FullscreenType::True);
        true
      }
      "borderless" => {
        let _ = window.set_fullscreen(sdl2::video::FullscreenType::Desktop);
        true
      }
      _ => false,
    };

    let mut canvas_builder = window.into_canvas();
    if app_cfg.display.vsync {
      canvas_builder = canvas_builder.present_vsync();
    }
    let mut canvas = canvas_builder.build()?;
    sdl_context.mouse().show_cursor(false);
    let events = sdl_context.event_pump().map_err(SdlError)?;
    let texture_creator = canvas.texture_creator();

    // Create texture we use as a permanent buffer for rendering, to make it easier to
    // replicate original game (this buffer is an equivalent of "video buffer").
    // This allows us to do additive rendering and do a "pallette animation" by blending it
    // with an alpha modifier on top of black screen.
    let buffer =
      texture_creator.create_texture_target(PixelFormatEnum::RGB24, SCREEN_WIDTH, SCREEN_HEIGHT)?;

    // Initialize audio
    sdl2::mixer::open_audio(44100, AUDIO_S16LSB, 2, 1024).map_err(SdlError)?;
    let ctx = ApplicationContext {
      game_dir,
      canvas: &mut canvas,
      events,
      buffer,
      texture_creator: &texture_creator,
      config: app_cfg,
      is_fullscreen,
    };
    cb(ctx)?;
    Ok(())
  }

  /// Invoke callback in a "rendering" context. Makes canvas to render in a separate buffer
  /// texture so we can apply post-processing to it (for example, emulate palette animation).
  pub fn with_render_context<R>(
    &mut self,
    callback: impl FnOnce(&mut WindowCanvas) -> Result<R, anyhow::Error>,
  ) -> Result<R, anyhow::Error> {
    let mut result = None;
    self.canvas.with_texture_canvas(&mut self.buffer, |canvas| {
      result = Some(callback(canvas));
    })?;
    result.unwrap()
  }

  pub fn render_texture(&mut self, texture: &Texture) -> Result<(), anyhow::Error> {
    self.with_render_context(|canvas| {
      canvas.copy(texture, None, None).map_err(SdlError)?;
      Ok(())
    })?;
    Ok(())
  }

  /// Load SPY texture from a given path
  pub fn load_spy(&self, file_name: &str) -> Result<TexturePalette<'textures>, anyhow::Error> {
    let path = self.game_dir.join(file_name);
    Ok(crate::images::load_texture(
      self.texture_creator,
      &path,
      TextureFormat::SPY,
    )?)
  }

  /// Load PPM texture from a given path
  pub fn load_ppm(&self, file_name: &str) -> Result<TexturePalette<'textures>, anyhow::Error> {
    let path = self.game_dir.join(file_name);
    Ok(crate::images::load_texture(
      self.texture_creator,
      &path,
      TextureFormat::PPM,
    )?)
  }

  /// Load fonts from a given path
  pub fn load_font(&self, file_name: &str) -> Result<Font<'textures>, anyhow::Error> {
    let path = self.game_dir.join(file_name);
    Ok(crate::fonts::load_font(self.texture_creator, &path)?)
  }

  pub fn load_music(&self, file_name: &str) -> Result<Music<'static>, anyhow::Error> {
    let path = self.game_dir.join(file_name);
    let music = Music::from_file(path).map_err(SdlError)?;
    Ok(music)
  }

  pub fn animate(&mut self, animation: Animation, steps: usize) -> Result<(), anyhow::Error> {
    // Note that we actually do steps + 1 iteration, as per original behavior
    // Roughly, we do it for half a second for 8 steps. For 60 FPS, which means ~4 frames per step.
    let total_frames = (steps + 1) * 4;

    for idx in 0..=total_frames {
      self.canvas.set_draw_color(Color::RGB(0, 0, 0));
      self.canvas.clear();
      let mut alpha = (255 * idx / total_frames) as u8;
      if let Animation::FadeDown = animation {
        alpha = 255 - alpha;
      }
      self.buffer.set_blend_mode(BlendMode::Blend);
      self.buffer.set_alpha_mod(alpha);
      self.canvas.copy(&self.buffer, None, None).map_err(SdlError)?;

      self.events.pump_events();
      self.canvas.present();
      self.wait_frame();
    }
    Ok(())
  }

  pub fn present(&mut self) -> Result<(), anyhow::Error> {
    self.present_shake(0)
  }

  pub fn present_shake(&mut self, shake: u16) -> Result<(), anyhow::Error> {
    self.buffer.set_blend_mode(BlendMode::None);
    self.buffer.set_alpha_mod(255);
    let (w, h) = self.canvas.output_size().map_err(SdlError)?;

    let (target_w, target_h, offset_x, offset_y) = if self.config.display.keep_aspect_ratio {
      let aspect = SCREEN_WIDTH as f32 / SCREEN_HEIGHT as f32;
      let win_aspect = w as f32 / h as f32;
      if win_aspect > aspect {
        // Window is wider than 4:3 -> pillarbox (black bars on left/right)
        let th = h;
        let tw = (h as f32 * aspect).round() as u32;
        let ox = ((w - tw) / 2) as i32;
        (tw, th, ox, 0)
      } else {
        // Window is taller than 4:3 -> letterbox (black bars on top/bottom)
        let tw = w;
        let th = (w as f32 / aspect).round() as u32;
        let oy = ((h - th) / 2) as i32;
        (tw, th, 0, oy)
      }
    } else {
      (w, h, 0, 0)
    };

    let mut target = Rect::new(offset_x, offset_y, target_w, target_h);

    // Clear canvas to black so letterboxing or shaking borders are black
    self.canvas.set_draw_color(Color::BLACK);
    self.canvas.clear();

    // Render "shaking" screen effect
    if shake != 0 {
      let top = u32::from(shake) * 10 * target_h / SCREEN_HEIGHT;
      target.set_y(offset_y - (top as i32));
    }
    self.canvas.copy(&self.buffer, None, Some(target)).map_err(SdlError)?;
    self.canvas.present();
    Ok(())
  }

  pub fn toggle_fullscreen(&mut self) -> Result<(), anyhow::Error> {
    let window = self.canvas.window_mut();
    if self.is_fullscreen {
      window.set_fullscreen(sdl2::video::FullscreenType::Off).map_err(SdlError)?;
      self.is_fullscreen = false;
      self.config.display.window_mode = "Windowed".to_string();
    } else {
      window.set_fullscreen(sdl2::video::FullscreenType::Desktop).map_err(SdlError)?;
      self.is_fullscreen = true;
      self.config.display.window_mode = "Borderless".to_string();
    }
    let _ = self.config.save(&self.game_dir);
    Ok(())
  }

  pub fn set_window_scale(&mut self, scale: u32) -> Result<(), anyhow::Error> {
    if scale == 0 {
      return Ok(());
    }
    let window = self.canvas.window_mut();
    if self.is_fullscreen {
      window.set_fullscreen(sdl2::video::FullscreenType::Off).map_err(SdlError)?;
      self.is_fullscreen = false;
      self.config.display.window_mode = "Windowed".to_string();
    }
    let new_w = SCREEN_WIDTH * scale;
    let new_h = SCREEN_HEIGHT * scale;
    window
      .set_size(new_w, new_h)
      .map_err(|e| anyhow::anyhow!("{}", e))?;
    window.set_position(sdl2::video::WindowPos::Centered, sdl2::video::WindowPos::Centered);
    self.config.display.scale = scale;
    self.config.display.width = new_w;
    self.config.display.height = new_h;
    let _ = self.config.save(&self.game_dir);
    Ok(())
  }

  pub fn toggle_aspect_ratio(&mut self) -> Result<(), anyhow::Error> {
    self.config.display.keep_aspect_ratio = !self.config.display.keep_aspect_ratio;
    let _ = self.config.save(&self.game_dir);
    Ok(())
  }

  pub fn present_flash(&mut self) -> Result<(), anyhow::Error> {
    self.canvas.set_draw_color(Color::WHITE);
    self.canvas.clear();
    self.canvas.present();
    Ok(())
  }

  pub fn wait_frame(&self) {
    let fps = if self.config.display.target_fps > 0 {
      self.config.display.target_fps.clamp(15, 240)
    } else {
      60
    };
    ::std::thread::sleep(Duration::from_nanos(1_000_000_000u64 / fps as u64));
  }

  /// Wait until some key is pressed
  pub fn wait_input_event(&mut self) -> InputEvent {
    loop {
      let event = self.events.wait_event();
      match event {
        // FIXME: proper event
        Event::Quit { .. } => return InputEvent::KeyPress(Scancode::Escape, Keycode::Escape),
        Event::KeyDown {
          scancode: Some(code),
          keycode: Some(key),
          repeat: false,
          ..
        } => return InputEvent::KeyPress(code, key),
        Event::TextInput { text, .. } => return InputEvent::TextInput(text),
        _ => {}
      }
    }
  }

  /// Wait until some key is pressed. This is a simpler interface for cases where we don't expect
  /// text input (most of the time, we don't)
  pub fn wait_key_pressed(&mut self) -> (Scancode, Keycode) {
    loop {
      if let InputEvent::KeyPress(scan, key) = self.wait_input_event() {
        return (scan, key);
      }
    }
  }

  pub fn poll_iter(&mut self) -> impl Iterator<Item = Event> + '_ {
    self.events.poll_iter()
  }

  pub fn game_dir(&self) -> &Path {
    &self.game_dir
  }

  pub fn texture_creator(&self) -> &'textures TextureCreator<WindowContext> {
    self.texture_creator
  }
}
