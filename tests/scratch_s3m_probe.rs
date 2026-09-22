//! Throwaway probe for Ubuntu's SDL2_mixer (libmodplug), not part of the real test suite.
//! Deleted before this branch's next commit. Windows already works via a different S3M path (libxmp);
//! this narrows down what Linux's ModPlug_Load additionally needs.
use std::io::Write;

fn base_header() -> Vec<u8> {
  let mut b = vec![0u8; 0x60];
  b[0..4].copy_from_slice(b"stub");
  b[0x1C] = 0x1A;
  b[0x1D] = 16;
  b[0x28..0x2A].copy_from_slice(&0x1300u16.to_le_bytes());
  b[0x2A..0x2C].copy_from_slice(&2u16.to_le_bytes());
  b[0x2C..0x30].copy_from_slice(b"SCRM");
  b[0x30] = 64;
  b[0x31] = 6;
  b[0x32] = 125;
  b[0x33] = 0x30;
  b[0x34] = 8;
  b[0x35] = 0x00;
  b[0x40] = 0x00;
  b[0x41] = 0x01;
  b
}

fn pad16(v: &mut Vec<u8>) {
  while v.len() % 16 != 0 {
    v.push(0);
  }
}

/// order count=1, pattern count=1, instrument count as given; each instrument is an 80-byte empty
/// (type 0) slot. One real pattern (64 empty rows) is always present.
fn s3m_with_instruments(n_instruments: u16, instrument_type: u8) -> Vec<u8> {
  let mut b = base_header();
  b[0x20..0x22].copy_from_slice(&1u16.to_le_bytes()); // order count
  b[0x22..0x24].copy_from_slice(&n_instruments.to_le_bytes());
  b[0x24..0x26].copy_from_slice(&1u16.to_le_bytes()); // pattern count

  let mut out = b;
  out.push(0); // order list: pattern 0
  pad16(&mut out);
  let ins_ptr_table = out.len();
  for _ in 0..n_instruments {
    out.push(0);
    out.push(0);
  }
  let pat_ptr_table = out.len();
  out.push(0);
  out.push(0);
  pad16(&mut out);

  // instruments (80 bytes each), each at its own paragraph boundary
  for i in 0..n_instruments {
    pad16(&mut out);
    let off = out.len();
    let para = (off / 16) as u16;
    out[ins_ptr_table + (i as usize) * 2..ins_ptr_table + (i as usize) * 2 + 2].copy_from_slice(&para.to_le_bytes());
    let mut ins = vec![0u8; 80];
    ins[0] = instrument_type;
    if instrument_type == 1 {
      ins[0x20..0x24].copy_from_slice(&8363u32.to_le_bytes()); // C2Spd
      ins[0x4C..0x50].copy_from_slice(b"SCRS");
    }
    out.extend_from_slice(&ins);
  }

  pad16(&mut out);
  let pat_off = out.len();
  let para = (pat_off / 16) as u16;
  out[pat_ptr_table..pat_ptr_table + 2].copy_from_slice(&para.to_le_bytes());
  let rows = vec![0u8; 64];
  out.extend_from_slice(&(rows.len() as u16).to_le_bytes());
  out.extend_from_slice(&rows);

  out
}

/// Same as s3m_with_instruments(0, _) but with the default-pan marker set and a full 32-byte pan table.
fn s3m_with_pan_table() -> Vec<u8> {
  let mut b = base_header();
  b[0x35] = 0xFC; // default pan marker: pan table follows the pointer tables
  b[0x20..0x22].copy_from_slice(&1u16.to_le_bytes());
  b[0x24..0x26].copy_from_slice(&1u16.to_le_bytes());

  let mut out = b;
  out.push(0);
  pad16(&mut out);
  let pat_ptr_table = out.len();
  out.push(0);
  out.push(0);
  pad16(&mut out);
  // 32-byte channel pan table (since default pan marker is 0xFC)
  out.extend_from_slice(&[0x08u8; 32]);
  pad16(&mut out);
  let pat_off = out.len();
  let para = (pat_off / 16) as u16;
  out[pat_ptr_table..pat_ptr_table + 2].copy_from_slice(&para.to_le_bytes());
  let rows = vec![0u8; 64];
  out.extend_from_slice(&(rows.len() as u16).to_le_bytes());
  out.extend_from_slice(&rows);
  out
}

#[test]
fn probe_variants() {
  std::env::set_var("SDL_AUDIODRIVER", "dummy");
  let sdl = sdl2::init().unwrap();
  let _audio = sdl.audio().unwrap();
  sdl2::mixer::open_audio(44100, sdl2::mixer::AUDIO_S16LSB, 2, 1024).unwrap();

  let variants: Vec<(&str, Vec<u8>)> = vec![
    ("baseline_0_instruments", s3m_with_instruments(0, 0)),
    ("1_empty_instrument", s3m_with_instruments(1, 0)),
    ("1_pcm_instrument_len0", s3m_with_instruments(1, 1)),
    ("4_empty_instruments", s3m_with_instruments(4, 0)),
    ("pan_table", s3m_with_pan_table()),
  ];

  let mut results = Vec::new();
  for (name, bytes) in variants {
    let path = std::env::temp_dir().join(format!("probe_{name}.s3m"));
    std::fs::File::create(&path).unwrap().write_all(&bytes).unwrap();
    let line = match sdl2::mixer::Music::from_file(&path) {
      Ok(_music) => format!("PROBE {name}: OK ({} bytes)", bytes.len()),
      Err(e) => format!("PROBE {name}: FAIL {e} ({} bytes)", bytes.len()),
    };
    results.push(line);
    let _ = std::fs::remove_file(&path);
  }
  // cargo test hides stdout for a passing test; force it into view by failing on purpose.
  panic!("\n{}", results.join("\n"));
}
