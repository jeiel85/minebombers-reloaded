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
