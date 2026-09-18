use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use num_enum::TryFromPrimitive;
use sdl2::keyboard::Scancode;
use std::convert::TryInto;
use std::path::Path;

#[derive(Default, Clone, Copy)]
pub struct KeyBindings {
  /// Keys, indexed by `Key` enum.
  keys: [Option<Scancode>; 8],
}

pub struct KeysConfig {
  /// Only 4 players for now
  pub keys: [KeyBindings; 4],
}

/// Key binding types. Note that this enum is ordered the same way we save them to the configuration
/// file and also the redefine menu.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, TryFromPrimitive, Debug)]
pub enum Key {
  Left,
  Right,
  Up,
  Down,
  Stop,
  Bomb,
  Choose,
  Remote,
}

impl Key {
  /// Iterate through the list of all key bindings
  pub fn all_keys() -> impl Iterator<Item = Key> {
    (0..8).map(|v| v.try_into().unwrap())
  }
}

impl std::ops::Index<Key> for KeyBindings {
  type Output = Option<Scancode>;

  fn index(&self, key: Key) -> &Self::Output {
    &self.keys[key as usize]
  }
}

impl std::ops::IndexMut<Key> for KeyBindings {
  fn index_mut(&mut self, key: Key) -> &mut Self::Output {
    &mut self.keys[key as usize]
  }
}

impl std::fmt::Display for Key {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let text = match self {
      Key::Left => "Left",
      Key::Right => "Right",
      Key::Up => "Up",
      Key::Down => "Down",
      Key::Stop => "Stop",
      Key::Bomb => "Bomb/Buy",
      Key::Choose => "Choose/Sell",
      Key::Remote => "Remote",
    };
    f.write_str(text)
  }
}

impl KeysConfig {
  /// Load key bindings from the configuration file. First, look for a new config (which can use all
  /// the scancodes that SDL supports). If not found, try loading old config. If old config is not
  /// found, just go with the defaults.
  pub fn load(game_dir: &Path) -> Self {
    let app_cfg = crate::config::AppConfig::load_or_create(game_dir);
    let mut config = KeysConfig { keys: default_keys() };
    app_cfg.apply_to_keys_config(&mut config);
    config
  }

  /// Save key bindings; saves to config.toml and legacy keysrel.cfg
  pub fn save(&self, game_dir: &Path) -> Result<(), anyhow::Error> {
    let mut app_cfg = crate::config::AppConfig::load_or_create(game_dir);
    app_cfg.update_from_keys_config(self);
    let _ = app_cfg.save(game_dir);

    let mut buf = Vec::with_capacity(32);
    for keys in self.keys.iter() {
      for key in Key::all_keys() {
        let value = keys[key].map(|k| k as i32).unwrap_or(0);
        buf.write_i32::<LittleEndian>(value)?;
      }
    }
    let file = game_dir.join("keysrel.cfg");
    let _ = std::fs::write(file, &buf);
    Ok(())
  }
}

fn default_keys() -> [KeyBindings; 4] {
  [
    KeyBindings {
      keys: [
        Some(Scancode::A),
        Some(Scancode::D),
        Some(Scancode::W),
        Some(Scancode::S),
        Some(Scancode::Z),
        Some(Scancode::Tab),
        Some(Scancode::LCtrl),
        Some(Scancode::LShift),
      ],
    },
    KeyBindings {
      keys: [
        Some(Scancode::J),
        Some(Scancode::L),
        Some(Scancode::I),
        Some(Scancode::K),
        Some(Scancode::Num7),
        Some(Scancode::Num0),
        Some(Scancode::Num8),
        Some(Scancode::Num9),
      ],
    },
    KeyBindings::default(),
    KeyBindings::default(),
  ]
}

#[allow(dead_code)]
/// Load key assignments from a new configuration file
fn load_keys_internal(path: &Path) -> Option<[KeyBindings; 4]> {
  let file = path.join("keysrel.cfg");
  let data = std::fs::read(file).ok()?;

  if data.len() != 128 {
    return None;
  }

  let mut it = data.as_slice();
  let mut keys: [KeyBindings; 4] = Default::default();
  for keys in keys.iter_mut() {
    for key in Key::all_keys() {
      let value = match it.read_i32::<LittleEndian>().unwrap() {
        0 => None,
        value => Scancode::from_i32(value),
      };
      keys[key] = value;
    }
  }
  Some(keys)
}

#[allow(dead_code)]
/// Load key assignments from an old configuration file
fn load_keys_legacy(path: &Path) -> Option<[KeyBindings; 4]> {
  let file = path.join("keys.cfg");
  let data = std::fs::read(file).ok()?;
  if data.len() != 32 {
    return None;
  }

  let mut it = data.as_slice();
  let mut keys: [KeyBindings; 4] = Default::default();
  for player in 0..4 {
    let keys = &mut keys[player];
    // Note that order is different from our `Key` enum, so we have to index individually
    keys[Key::Up] = from_legacy_scancode(it.read_u8().unwrap());
    keys[Key::Down] = from_legacy_scancode(it.read_u8().unwrap());
    keys[Key::Left] = from_legacy_scancode(it.read_u8().unwrap());
    keys[Key::Right] = from_legacy_scancode(it.read_u8().unwrap());
    keys[Key::Bomb] = from_legacy_scancode(it.read_u8().unwrap());
    keys[Key::Remote] = from_legacy_scancode(it.read_u8().unwrap());
    keys[Key::Choose] = from_legacy_scancode(it.read_u8().unwrap());
    keys[Key::Stop] = from_legacy_scancode(it.read_u8().unwrap());
  }
  Some(keys)
}

fn from_legacy_scancode(key: u8) -> Option<Scancode> {
  let key = usize::from(key);
  if key < MAPPING.len() {
    Some(MAPPING[key])
  } else {
    None
  }
}

/// Map all non-supported codes to this value
const NOT_MAPPED: Scancode = Scancode::Application;

/// Mapping between scancodes used by original game and SDL scancodes
const MAPPING: [Scancode; 0x54] = [
  NOT_MAPPED,
  Scancode::Escape,
  Scancode::Num1,
  Scancode::Num2,
  Scancode::Num3,
  Scancode::Num4,
  Scancode::Num5,
  Scancode::Num6,
  Scancode::Num7,
  Scancode::Num8,
  Scancode::Num9,
  Scancode::Num0,
  Scancode::Minus,
  Scancode::Equals,
  Scancode::Backspace,
  Scancode::Tab,
  Scancode::Q,
  Scancode::W,
  Scancode::E,
  Scancode::R,
  Scancode::T,
  Scancode::Y,
  Scancode::U,
  Scancode::I,
  Scancode::O,
  Scancode::P,
  Scancode::LeftBracket,
  Scancode::RightBracket,
  Scancode::Return,
  Scancode::LCtrl,
  Scancode::A,
  Scancode::S,
  Scancode::D,
  Scancode::F,
  Scancode::G,
  Scancode::H,
  Scancode::J,
  Scancode::K,
  Scancode::L,
  Scancode::Semicolon,
  Scancode::Apostrophe,
  Scancode::Grave,
  Scancode::LShift,
  Scancode::Backslash,
  Scancode::Z,
  Scancode::X,
  Scancode::C,
  Scancode::V,
  Scancode::B,
  Scancode::N,
  Scancode::M,
  Scancode::Comma,
  Scancode::Period,
  Scancode::Slash,
  Scancode::RShift,
  // `*`
  NOT_MAPPED,
  Scancode::LAlt,
  Scancode::Space,
  Scancode::CapsLock,
  Scancode::F1,
  Scancode::F2,
  Scancode::F3,
  Scancode::F4,
  Scancode::F5,
  Scancode::F6,
  Scancode::F7,
  Scancode::F8,
  Scancode::F9,
  Scancode::F10,
  Scancode::NumLockClear,
  Scancode::ScrollLock,
  Scancode::Kp7,
  Scancode::Kp8,
  Scancode::Kp9,
  Scancode::KpMinus,
  Scancode::Kp4,
  Scancode::Kp5,
  Scancode::Kp6,
  Scancode::KpPlus,
  Scancode::Kp1,
  Scancode::Kp2,
  Scancode::Kp3,
  Scancode::Kp0,
  Scancode::KpDecimal,
];
