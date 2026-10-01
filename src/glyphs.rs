//! Rendering of the game glyph texture. Which glyph sits where in that texture is shared with the web
//! edition and lives in `mb_core::glyphs`.
use crate::error::ApplicationError::SdlError;
use crate::images::TexturePalette;
pub use mb_core::glyphs::*;
use sdl2::rect::Rect;
use sdl2::render::{Texture, WindowCanvas};

/// Glyphs is one single texture with all game icons on it.
pub struct Glyphs<'t> {
  texture: Texture<'t>,
}

impl<'t> Glyphs<'t> {
  /// Load glyph texture
  pub fn from_texture(texture: TexturePalette<'t>) -> Glyphs<'t> {
    Self {
      texture: texture.texture,
    }
  }

  /// Render given glyph at position
  pub fn render(&self, canvas: &mut WindowCanvas, x: i32, y: i32, glyph: Glyph) -> Result<(), anyhow::Error> {
    let src = glyph.rect();
    let src_rect = Rect::new(src.x, src.y, src.w, src.h);
    let tgt_rect = Rect::new(x, y, src.w, src.h);
    canvas.copy(&self.texture, src_rect, tgt_rect).map_err(SdlError)?;
    Ok(())
  }
}
