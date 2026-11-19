//! Rendering of the game glyph texture. Which glyph sits where in that texture is shared with the web
//! edition and lives in `mb_core::glyphs`.
use crate::error::ApplicationError::SdlError;
use crate::images::TexturePalette;
pub use mb_core::glyphs::*;
use sdl2::rect::Rect;
use sdl2::render::{Texture, WindowCanvas};

/// Glyphs is one single texture with all game icons on it.
///
/// Two copies of the sheet are kept: `opaque` for map tiles and their edge overlays, and
/// `transparent` with exact black keyed out for sprites. `Glyph::is_transparent` picks between
/// them, and the web edition reads the same rule so both look alike.
pub struct Glyphs<'t> {
  opaque: Texture<'t>,
  transparent: Texture<'t>,
}

impl<'t> Glyphs<'t> {
  /// Load glyph textures: the sheet as stored, and the same sheet with exact black keyed out
  pub fn from_textures(opaque: TexturePalette<'t>, transparent: TexturePalette<'t>) -> Glyphs<'t> {
    Self {
      opaque: opaque.texture,
      transparent: transparent.texture,
    }
  }

  /// Render given glyph at position
  pub fn render(&self, canvas: &mut WindowCanvas, x: i32, y: i32, glyph: Glyph) -> Result<(), anyhow::Error> {
    let texture = if glyph.is_transparent() {
      &self.transparent
    } else {
      &self.opaque
    };
    let src = glyph.rect();
    let src_rect = Rect::new(src.x, src.y, src.w, src.h);
    let tgt_rect = Rect::new(x, y, src.w, src.h);
    canvas.copy(texture, src_rect, tgt_rect).map_err(SdlError)?;
    Ok(())
  }
}
