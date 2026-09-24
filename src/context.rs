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
use std::time::{Duration, Instant};

/// Application environment resources packaged into one structs. Provides helper functions used
/// across the whole application.
pub struct ApplicationContext<'canvas, 'textures> {
  pub game_dir: PathBuf,
  /// Where the engine writes its files; the game folder itself is only read
  pub user_dir: PathBuf,
  pub events: EventPump,
  pub canvas: &'canvas mut WindowCanvas,
  pub buffer: Texture<'textures>,
  pub texture_creator: &'textures TextureCreator<WindowContext>,
  pub config: crate::config::AppConfig,
  pub is_fullscreen: bool,
  pub gamepad: crate::gamepad::GamepadManager,
  pub crt_texture: Texture<'textures>,
  pub light_mask: Texture<'textures>,
  pub lantern_light: Texture<'textures>,
  pub explosion_light: Texture<'textures>,
  pub lighting_active: bool,
  /// Short on-screen message (game speed, toggles, pause), drawn over the frame at present time so
  /// it never touches `buffer`, which the game only redraws where something changed.
  osd: Texture<'textures>,
  osd_visible: OsdVisible,
}

/// How long the on-screen message stays up
#[derive(Clone, Copy)]
enum OsdVisible {
  Hidden,
  Until(Instant),
  /// Until `hide_osd`
  Sticky,
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

    let user_dir = crate::userdata::prepare(&game_dir);
    let app_cfg = crate::config::AppConfig::load_or_create(&user_dir);
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

    // Create static CRT scanline and vignette overlay texture
    let crt_texture = create_crt_texture(&texture_creator)?;

    // Create dynamic lighting mask and light textures
    let mut light_mask =
      texture_creator.create_texture_target(PixelFormatEnum::RGBA32, SCREEN_WIDTH, SCREEN_HEIGHT)?;
    light_mask.set_blend_mode(BlendMode::Mod);

    let lantern_light = create_radial_light(&texture_creator, 128, (230, 205, 140))?;
    let explosion_light = create_radial_light(&texture_creator, 256, (255, 190, 90))?;

    let mut osd = texture_creator.create_texture_target(PixelFormatEnum::RGBA32, SCREEN_WIDTH, SCREEN_HEIGHT)?;
    osd.set_blend_mode(BlendMode::Blend);

    // Initialize audio
    sdl2::mixer::open_audio(44100, AUDIO_S16LSB, 2, 1024).map_err(SdlError)?;

    // Initialize game controller subsystem
    let controller_subsystem = sdl_context.game_controller().map_err(SdlError)?;
    let gamepad = crate::gamepad::GamepadManager::new(controller_subsystem);

    let ctx = ApplicationContext {
      game_dir,
      user_dir,
      canvas: &mut canvas,
      events,
      buffer,
      texture_creator: &texture_creator,
      config: app_cfg,
      is_fullscreen,
      gamepad,
      crt_texture,
      light_mask,
      lantern_light,
      explosion_light,
      lighting_active: false,
      osd,
      osd_visible: OsdVisible::Hidden,
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

    // Apply dynamic cave lighting if enabled and active
    if self.config.graphics.dynamic_lighting && self.lighting_active {
      let _ = self.light_mask.set_blend_mode(BlendMode::Mod);
      self.canvas.copy(&self.light_mask, None, Some(target)).map_err(SdlError)?;
      self.lighting_active = false;
    }

    let osd_on = match self.osd_visible {
      OsdVisible::Hidden => false,
      OsdVisible::Until(until) => Instant::now() < until,
      OsdVisible::Sticky => true,
    };
    if osd_on {
      self.canvas.copy(&self.osd, None, Some(target)).map_err(SdlError)?;
    }

    // Apply retro CRT scanline & vignette filter if enabled
    if self.config.graphics.crt_shader {
      self.canvas.copy(&self.crt_texture, None, Some(target)).map_err(SdlError)?;
    }

    self.canvas.present();
    Ok(())
  }

  /// Show `text` centered just below the top status bar (the next frames presented with
  /// `present`/`present_shake` draw it). `duration` of `None` keeps it up until `hide_osd`.
  pub fn show_osd(&mut self, font: &Font, text: &str, duration: Option<Duration>) -> Result<(), anyhow::Error> {
    const TOP: i32 = 44;
    let width = 8 * text.chars().count() as u32 + 16;
    let left = (SCREEN_WIDTH as i32 - width as i32) / 2;
    let mut result = Ok(());
    self.canvas.with_texture_canvas(&mut self.osd, |canvas| {
      canvas.set_draw_color(Color::RGBA(0, 0, 0, 0));
      canvas.clear();
      canvas.set_blend_mode(BlendMode::None);
      canvas.set_draw_color(Color::RGBA(0, 0, 0, 190));
      result = canvas
        .fill_rect(Rect::new(left, TOP, width, 16))
        .map_err(|e| anyhow::Error::from(SdlError(e)))
        .and_then(|()| font.render(canvas, left + 8, TOP + 4, Color::RGB(255, 230, 90), text));
    })?;
    result?;
    self.osd_visible = match duration {
      Some(duration) => OsdVisible::Until(Instant::now() + duration),
      None => OsdVisible::Sticky,
    };
    Ok(())
  }

  pub fn hide_osd(&mut self) {
    self.osd_visible = OsdVisible::Hidden;
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
    let _ = self.config.save(&self.user_dir);
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
    let _ = self.config.save(&self.user_dir);
    Ok(())
  }

  pub fn toggle_aspect_ratio(&mut self) -> Result<(), anyhow::Error> {
    self.config.display.keep_aspect_ratio = !self.config.display.keep_aspect_ratio;
    let _ = self.config.save(&self.user_dir);
    Ok(())
  }

  pub fn present_flash(&mut self) -> Result<(), anyhow::Error> {
    self.lighting_active = false;
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
      self.gamepad.handle_event(&event);
      match event {
        Event::Quit { .. } => {
          std::process::exit(0);
        }
        Event::ControllerButtonDown { button, .. } => {
          use sdl2::controller::Button;
          match button {
            Button::DPadUp => return InputEvent::KeyPress(Scancode::Up, Keycode::Up),
            Button::DPadDown => return InputEvent::KeyPress(Scancode::Down, Keycode::Down),
            Button::DPadLeft => return InputEvent::KeyPress(Scancode::Left, Keycode::Left),
            Button::DPadRight => return InputEvent::KeyPress(Scancode::Right, Keycode::Right),
            Button::A | Button::Start => return InputEvent::KeyPress(Scancode::Return, Keycode::Return),
            Button::B | Button::Back => return InputEvent::KeyPress(Scancode::Escape, Keycode::Escape),
            Button::X | Button::Y | Button::LeftShoulder | Button::RightShoulder => {
              return InputEvent::KeyPress(Scancode::Tab, Keycode::Tab)
            }
            _ => {}
          }
        }
        Event::ControllerAxisMotion { axis, value, .. } if value.abs() > 16000 => {
          use sdl2::controller::Axis;
          match axis {
            Axis::LeftY if value < -16000 => return InputEvent::KeyPress(Scancode::Up, Keycode::Up),
            Axis::LeftY if value > 16000 => return InputEvent::KeyPress(Scancode::Down, Keycode::Down),
            Axis::LeftX if value < -16000 => return InputEvent::KeyPress(Scancode::Left, Keycode::Left),
            Axis::LeftX if value > 16000 => return InputEvent::KeyPress(Scancode::Right, Keycode::Right),
            _ => {}
          }
        }
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

  pub fn poll_events(&mut self) -> Vec<Event> {
    let events: Vec<Event> = self.events.poll_iter().collect();
    for event in &events {
      self.gamepad.handle_event(event);
    }
    events
  }

  pub fn game_dir(&self) -> &Path {
    &self.game_dir
  }

  pub fn user_dir(&self) -> &Path {
    &self.user_dir
  }

  pub fn texture_creator(&self) -> &'textures TextureCreator<WindowContext> {
    self.texture_creator
  }

  pub fn toggle_crt(&mut self) -> bool {
    self.config.graphics.crt_shader = !self.config.graphics.crt_shader;
    println!(
      "[GRAPHICS] CRT Scanline & Vignette Filter: {}",
      if self.config.graphics.crt_shader { "ENABLED" } else { "DISABLED" }
    );
    let _ = self.config.save(&self.user_dir);
    self.config.graphics.crt_shader
  }

  pub fn toggle_dynamic_lighting(&mut self) -> bool {
    self.config.graphics.dynamic_lighting = !self.config.graphics.dynamic_lighting;
    println!(
      "[GRAPHICS] Dynamic Cave Lighting & Lanterns: {}",
      if self.config.graphics.dynamic_lighting { "ENABLED" } else { "DISABLED" }
    );
    let _ = self.config.save(&self.user_dir);
    self.config.graphics.dynamic_lighting
  }

  pub fn apply_dynamic_lighting(
    &mut self,
    lanterns: &[(u16, u16)],
    explosions: &[(u16, u16, f32)],
  ) -> Result<(), anyhow::Error> {
    if !self.config.graphics.dynamic_lighting {
      return Ok(());
    }

    self.lighting_active = true;
    let lantern = &self.lantern_light;
    let explosion = &self.explosion_light;

    self.canvas.with_texture_canvas(&mut self.light_mask, |canvas| {
      // Subterranean darkness: dim bluish-gray ambient light
      canvas.set_draw_color(Color::RGB(55, 55, 70));
      canvas.clear();

      // Top HUD (scores, lives) and bottom HUD (round timer) are always 100% visible
      canvas.set_draw_color(Color::RGB(255, 255, 255));
      let _ = canvas.fill_rect(Rect::new(0, 0, SCREEN_WIDTH, 38));
      let _ = canvas.fill_rect(Rect::new(0, 470, SCREEN_WIDTH, 10));

      // Draw warm lantern light for each miner
      for &(lx, ly) in lanterns {
        let x = lx as i32 - 64;
        let y = ly as i32 - 64;
        let _ = canvas.copy(lantern, None, Some(Rect::new(x, y, 128, 128)));
      }

      // Draw explosion blast radiance
      for &(ex, ey, intensity) in explosions {
        let size = (256.0 * intensity.clamp(0.4, 1.5)) as u32;
        let half = (size / 2) as i32;
        let x = ex as i32 - half;
        let y = ey as i32 - half;
        let _ = canvas.copy(explosion, None, Some(Rect::new(x, y, size, size)));
      }
    })?;

    Ok(())
  }
}

fn create_crt_texture<'textures>(
  texture_creator: &'textures TextureCreator<WindowContext>,
) -> Result<Texture<'textures>, anyhow::Error> {
  let mut tex = texture_creator
    .create_texture_static(PixelFormatEnum::RGBA32, SCREEN_WIDTH, SCREEN_HEIGHT)
    .map_err(|e| anyhow::anyhow!("{}", e))?;
  tex.set_blend_mode(BlendMode::Blend);

  let mut pixels = vec![0u8; (SCREEN_WIDTH * SCREEN_HEIGHT * 4) as usize];
  let center_x = SCREEN_WIDTH as f32 / 2.0;
  let center_y = SCREEN_HEIGHT as f32 / 2.0;
  let max_dist = (center_x * center_x + center_y * center_y).sqrt();

  for y in 0..SCREEN_HEIGHT {
    let scanline = if y % 2 == 1 { 0.40 } else { 0.0 };

    for x in 0..SCREEN_WIDTH {
      let dx = x as f32 - center_x;
      let dy = y as f32 - center_y;
      let dist = (dx * dx + dy * dy).sqrt() / max_dist;

      let vignette = (dist * dist * 0.55).min(0.75);
      let alpha = ((scanline + vignette).min(0.85) * 255.0) as u8;

      let idx = ((y * SCREEN_WIDTH + x) * 4) as usize;
      pixels[idx] = 0;
      pixels[idx + 1] = 0;
      pixels[idx + 2] = 0;
      pixels[idx + 3] = alpha;
    }
  }

  tex.update(None, &pixels, (SCREEN_WIDTH * 4) as usize).map_err(|e| anyhow::anyhow!("{}", e))?;
  Ok(tex)
}

fn create_radial_light<'textures>(
  texture_creator: &'textures TextureCreator<WindowContext>,
  size: u32,
  base_color: (u8, u8, u8),
) -> Result<Texture<'textures>, anyhow::Error> {
  let mut tex = texture_creator
    .create_texture_static(PixelFormatEnum::RGBA32, size, size)
    .map_err(|e| anyhow::anyhow!("{}", e))?;
  tex.set_blend_mode(BlendMode::Add);

  let mut pixels = vec![0u8; (size * size * 4) as usize];
  let center = size as f32 / 2.0;
  let radius = center;

  for y in 0..size {
    for x in 0..size {
      let dx = x as f32 - center;
      let dy = y as f32 - center;
      let dist = (dx * dx + dy * dy).sqrt();

      if dist < radius {
        let factor = (1.0 - (dist / radius)).powi(2);
        let r = (base_color.0 as f32 * factor) as u8;
        let g = (base_color.1 as f32 * factor) as u8;
        let b = (base_color.2 as f32 * factor) as u8;

        let idx = ((y * size + x) * 4) as usize;
        pixels[idx] = r;
        pixels[idx + 1] = g;
        pixels[idx + 2] = b;
        pixels[idx + 3] = 255;
      }
    }
  }

  tex.update(None, &pixels, (size * 4) as usize).map_err(|e| anyhow::anyhow!("{}", e))?;
  Ok(tex)
}
