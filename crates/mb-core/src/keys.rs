use num_enum::TryFromPrimitive;
use std::convert::TryInto;

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
  pub fn all_keys() -> impl Iterator<Item = Key> {
    (0..8).map(|v| v.try_into().unwrap())
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

/// Key bindings of one player as raw platform key codes, indexed by `Key`. The game world only
/// stores these; each front end decides what the codes mean (the native game uses SDL scancodes).
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyBindings {
  pub raw_codes: [u32; 8],
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeysConfig {
  pub keys: [KeyBindings; 4],
}

impl KeysConfig {
  pub fn load(_game_dir: &std::path::Path) -> Self {
    Self::default()
  }
}
