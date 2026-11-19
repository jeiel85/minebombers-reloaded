use thiserror::Error;

pub const SCREEN_WIDTH: u32 = 640;
pub const SCREEN_HEIGHT: u32 = 480;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

impl Color {
  pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
  pub const WHITE: Color = Color { r: 255, g: 255, b: 255 };
  pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
    Color { r, g, b }
  }
}

#[derive(Debug, Error)]
#[error("Provided SPY file is not in a valid SPY file format")]
pub struct InvalidSpyFile;

#[derive(Debug, Error)]
#[error("Provided PPM file is not in a valid PPM file format")]
pub struct InvalidPpmFile;

#[derive(Debug, Error)]
#[error("Provided FON file is not in a valid FON file format")]
pub struct InvalidFontFile;

pub fn decode_font(data: &[u8]) -> Result<Vec<u8>, InvalidFontFile> {
  if data.len() != 256 * 8 {
    return Err(InvalidFontFile);
  }
  let mut image = Vec::with_capacity(256 * 8 * 8 * 4);
  for row in 0..16 {
    for glyph_line in 0..8 {
      for col in 0..16 {
        for bit in 0..8 {
          let mask = 1 << (7 - bit);
          let value = data[(row * 16 + col) * 8 + glyph_line];
          if (value & mask) != 0 {
            image.push(255);
            image.push(255);
            image.push(255);
            image.push(255);
          } else {
            image.push(0);
            image.push(0);
            image.push(0);
            image.push(0);
          }
        }
      }
    }
  }
  Ok(image)
}

/// Raw data for the decoded image.
pub struct DecodedImage {
  pub width: u32,
  pub height: u32,
  pub palette: [Color; 16],
  /// Image bytes, 3 bytes per pixel, RGB.
  pub image: Vec<u8>,
}

pub fn decode_spy(width: u32, height: u32, data: &[u8]) -> Result<DecodedImage, InvalidSpyFile> {
  let bitplane_len = (width as usize) * (height as usize) / 8;
  if data.len() < 768 {
    return Err(InvalidSpyFile);
  }

  let (palette, data) = data.split_at(768);
  let mut it = data.iter().copied();

  let plane0 = decode_plane(bitplane_len, &mut it)?;
  let plane1 = decode_plane(bitplane_len, &mut it)?;
  let plane2 = decode_plane(bitplane_len, &mut it)?;
  let plane3 = decode_plane(bitplane_len, &mut it)?;

  let mut image = Vec::with_capacity(bitplane_len * 24);
  for idx in 0..bitplane_len {
    for bit in (0..8).rev() {
      let bit0 = (plane0[idx] >> bit) & 1;
      let bit1 = ((plane1[idx] >> bit) & 1) << 1;
      let bit2 = ((plane2[idx] >> bit) & 1) << 2;
      let bit3 = ((plane3[idx] >> bit) & 1) << 3;
      let color = (bit0 | bit1 | bit2 | bit3) as usize;

      image.push(palette[color * 3]);
      image.push(palette[color * 3 + 1]);
      image.push(palette[color * 3 + 2]);
    }
  }
  Ok(DecodedImage {
    width,
    height,
    palette: decode_palette(palette),
    image,
  })
}

/// Inverse of [`decode_spy`]: a 768-byte palette (the first 16 colours, the rest zero) followed by
/// four RLE-encoded bitplanes. Every pixel must match one of the 16 palette colours exactly, and
/// `width * height` must be a multiple of 8. Anything the decoder could not read back is rejected
/// with `InvalidSpyFile`.
pub fn encode_spy(image: &DecodedImage) -> Result<Vec<u8>, InvalidSpyFile> {
  let width = image.width as usize;
  let height = image.height as usize;
  let pixels = width
    .checked_mul(height)
    .filter(|count| *count > 0 && count % 8 == 0)
    .ok_or(InvalidSpyFile)?;
  if image.image.len() != pixels * 3 {
    return Err(InvalidSpyFile);
  }

  let mut indices = Vec::with_capacity(pixels);
  for px in image.image.chunks_exact(3) {
    let color = Color {
      r: px[0],
      g: px[1],
      b: px[2],
    };
    let index = image
      .palette
      .iter()
      .position(|entry| *entry == color)
      .ok_or(InvalidSpyFile)?;
    indices.push(index as u8);
  }

  let mut out = Vec::with_capacity(768 + pixels / 8 * 4);
  for color in &image.palette {
    out.push(color.r);
    out.push(color.g);
    out.push(color.b);
  }
  out.resize(768, 0);

  for plane in 0..4 {
    let mut raw = Vec::with_capacity(pixels / 8);
    for chunk in indices.chunks_exact(8) {
      let mut byte = 0u8;
      for (bit, index) in chunk.iter().enumerate() {
        byte |= ((index >> plane) & 1) << (7 - bit);
      }
      raw.push(byte);
    }
    encode_plane(&raw, &mut out);
  }
  Ok(out)
}

/// Emit one bitplane exactly as `decode_plane` reads it. A literal `0x01` must be escaped, and runs
/// of four or more bytes are shorter as `01 value count`.
fn encode_plane(raw: &[u8], out: &mut Vec<u8>) {
  let mut index = 0;
  while index < raw.len() {
    let value = raw[index];
    let mut run = 1;
    while index + run < raw.len() && raw[index + run] == value && run < usize::from(u8::MAX) {
      run += 1;
    }
    if run >= 4 || value == 1 {
      out.push(1);
      out.push(value);
      out.push(run as u8);
    } else {
      out.extend(std::iter::repeat(value).take(run));
    }
    index += run;
  }
}

fn decode_plane(bitplane_len: usize, mut it: impl Iterator<Item = u8>) -> Result<Vec<u8>, InvalidSpyFile> {
  let mut image = Vec::new();
  while image.len() < bitplane_len {
    let val = it.next().ok_or(InvalidSpyFile)?;
    if val != 1 {
      image.push(val);
    } else {
      let val = it.next().ok_or(InvalidSpyFile)?;
      let len = it.next().ok_or(InvalidSpyFile)?;
      for _ in 0..len {
        image.push(val);
      }
    }
  }
  Ok(image)
}

pub fn decode_ppm(data: &[u8]) -> Result<DecodedImage, InvalidPpmFile> {
  if data.len() < 128 + 768 + 1 {
    return Err(InvalidPpmFile);
  }
  let from_y = u32::from(data[6]) + (u32::from(data[7]) << 8);
  let to_y = u32::from(data[10]) + (u32::from(data[11]) << 8);
  let width = u32::from(data[0x42]) + (u32::from(data[0x43]) << 8);
  let height = to_y - from_y;
  let mut it = data[128..data.len() - 769].iter().copied();
  let palette = &data[data.len() - 768..];

  let mut image = Vec::with_capacity((width as usize) * (height as usize) * 3);
  for _ in 0..height {
    let mut x = 0;
    while x < width {
      let value = it.next().ok_or(InvalidPpmFile)?;
      if (value & 0xC0) == 0xC0 {
        let len = u32::from(value) & 0x3F;
        let color = usize::from(it.next().ok_or(InvalidPpmFile)?);

        if x + len > width {
          return Err(InvalidPpmFile);
        }
        for _ in 0..len {
          image.push(palette[color * 3]);
          image.push(palette[color * 3 + 1]);
          image.push(palette[color * 3 + 2]);
        }
        x += len;
      } else {
        let color = usize::from(value);
        image.push(palette[color * 3]);
        image.push(palette[color * 3 + 1]);
        image.push(palette[color * 3 + 2]);
        x += 1;
      }
    }
  }

  Ok(DecodedImage {
    width,
    height,
    palette: decode_palette(palette),
    image,
  })
}

fn decode_palette(data: &[u8]) -> [Color; 16] {
  let mut palette: [Color; 16] = [Color::BLACK; 16];
  for color in 0..16 {
    let r = data[color * 3];
    let g = data[color * 3 + 1];
    let b = data[color * 3 + 2];
    palette[color] = Color::rgb(r, g, b);
  }
  palette
}

#[cfg(test)]
mod tests {
  use super::*;

  /// 16 distinct colours; the odd offsets make an accidental match with black impossible.
  fn palette() -> [Color; 16] {
    let mut palette = [Color::BLACK; 16];
    for (index, color) in palette.iter_mut().enumerate() {
      *color = Color::rgb(index as u8 * 16, index as u8 * 16 + 1, index as u8 * 16 + 2);
    }
    palette
  }

  #[test]
  fn spy_encoder_round_trips_every_plane_and_palette_entry() {
    let width = 16;
    let height = 8;
    let palette = palette();
    let mut image = Vec::with_capacity(width as usize * height as usize * 3);
    for row in 0..height {
      for col in 0..width {
        let index = if row == 0 { col % 16 } else { (col + row * 3) % 16 };
        let color = palette[index as usize];
        image.extend_from_slice(&[color.r, color.g, color.b]);
      }
    }

    let encoded = encode_spy(&DecodedImage {
      width,
      height,
      palette,
      image: image.clone(),
    })
    .unwrap();
    let decoded = decode_spy(width, height, &encoded).unwrap();
    assert_eq!(decoded.width, width);
    assert_eq!(decoded.height, height);
    assert_eq!(decoded.palette, palette);
    assert_eq!(decoded.image, image);
  }

  #[test]
  fn spy_encoder_escapes_a_literal_0x01_plane_byte() {
    let width = 8;
    let height = 1;
    let palette = palette();
    // Plane 0's only byte is 0b00000001. The encoder must write `01 01 01`; a plain `01` would be
    // read back as the start of an RLE command.
    let mut image = Vec::new();
    for index in [0usize, 0, 0, 0, 0, 0, 0, 1] {
      let color = palette[index];
      image.extend_from_slice(&[color.r, color.g, color.b]);
    }

    let encoded = encode_spy(&DecodedImage {
      width,
      height,
      palette,
      image: image.clone(),
    })
    .unwrap();
    assert_eq!(&encoded[768..771], &[1, 1, 1]);
    let decoded = decode_spy(width, height, &encoded).unwrap();
    assert_eq!(decoded.image, image);
  }

  #[test]
  fn spy_encoder_rejects_a_mismatched_buffer() {
    let image = DecodedImage {
      width: 16,
      height: 8,
      palette: palette(),
      image: vec![0; 3],
    };
    assert!(encode_spy(&image).is_err());
  }

  #[test]
  fn spy_encoder_rejects_a_colour_outside_the_palette() {
    let image = DecodedImage {
      width: 8,
      height: 1,
      palette: palette(),
      image: [1u8, 2, 3].repeat(8),
    };
    assert!(encode_spy(&image).is_err());
  }
}
