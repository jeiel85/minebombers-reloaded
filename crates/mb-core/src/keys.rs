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
