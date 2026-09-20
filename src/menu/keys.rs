use crate::context::{Animation, ApplicationContext};
use crate::error::ApplicationError::SdlError;
use crate::keys::{Key, KeysConfig};
use crate::Application;
use sdl2::keyboard::Scancode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;
use std::convert::TryInto;

impl Application<'_> {
  pub fn redefine_keys_menu(
    &self,
    ctx: &mut ApplicationContext,
    keys_config: &mut KeysConfig,
  ) -> Result<(), anyhow::Error> {
    ctx.with_render_context(|canvas| {
      canvas.copy(&self.keys.texture, None, None).map_err(SdlError)?;
      self.render_configured_keys(canvas, keys_config)?;
      let hint_color = self.keys.palette[8];
      self.font.render(canvas, 110, 440, hint_color, "PRESS KEY TO BIND | ESC TO SKIP | F10 TO FINISH & SAVE")?;
      Ok(())
    })?;
    ctx.animate(Animation::FadeUp, 7)?;

    let color = self.keys.palette[5];
    let highlight_color = self.keys.palette[12];
    'outer: for player in 0..4 {
      for key in Key::all_keys() {
        let y = key_pos_y(player, key);

        // Highlight the current key slot so the user knows what is being asked
        ctx.with_render_context(|canvas| {
          canvas.set_draw_color(Color::BLACK);
          let rect = Rect::new(356, y, 144, 8);
          canvas.fill_rect(rect).map_err(SdlError)?;
          self.font.render(canvas, 356, y, highlight_color, "? PRESS KEY ?")?;
          Ok(())
        })?;
        ctx.present()?;

        let (scan, _) = ctx.wait_key_pressed();
        if scan == Scancode::F10 {
          // Restore display before breaking
          ctx.with_render_context(|canvas| {
            canvas.set_draw_color(Color::BLACK);
            let rect = Rect::new(356, y, 144, 8);
            canvas.fill_rect(rect).map_err(SdlError)?;
            if let Some(scancode) = keys_config.keys[player][key] {
              self.font.render(canvas, 356, y, color, &scancode.name().to_uppercase())?;
            }
            Ok(())
          })?;
          ctx.present()?;
          break 'outer;
        }
        if scan != Scancode::Escape {
          keys_config.keys[player][key] = Some(scan);
        }

        // Re-render the key
        ctx.with_render_context(|canvas| {
          canvas.set_draw_color(Color::BLACK);
          let rect = Rect::new(356, y, 144, 8);
          canvas.fill_rect(rect).map_err(SdlError)?;
          if let Some(scancode) = keys_config.keys[player][key] {
            self.font.render(canvas, 356, y, color, &scancode.name().to_uppercase())?;
          }
          Ok(())
        })?;
        ctx.present()?;
      }
    }

    // Save all assigned keys to config.toml and legacy cfg
    keys_config.save(ctx.user_dir())?;
    ctx.config.update_from_keys_config(keys_config);
    let _ = ctx.config.save(ctx.user_dir());
    ctx.animate(Animation::FadeDown, 7)?;
    Ok(())
  }

  fn render_configured_keys(&self, canvas: &mut WindowCanvas, keys_config: &KeysConfig) -> Result<(), anyhow::Error> {
    const COLORS: [usize; 3] = [12, 4, 8];
    const OFFSETS: [i32; 3] = [-1, 1, 0];
    for player in 0..4 {
      for layer in 0..3 {
        for key in 0..8 {
          let key: Key = key.try_into().unwrap();
          let keys = &keys_config.keys[player];
          let color = self.keys.palette[COLORS[layer]];

          let y = key_pos_y(player, key);
          let text = format!("Player {} {:11}: ", player + 1, key);
          self.font.render(canvas, 180 + OFFSETS[layer], y, color, &text)?;

          // Don't render "shadow" for keys
          if layer == 2 {
            if let Some(scancode) = keys[key] {
              self
                .font
                .render(canvas, 356, y, color, &scancode.name().to_uppercase())?;
            }
          }
        }
      }
    }
    Ok(())
  }
}

fn key_pos_y(player: usize, key: Key) -> i32 {
  (player as i32) * 80 + 10 * (key as i32) + 100
}
