//! Configuration file management for Mine Bombers
//!
//! Handles `config.toml` for display settings, FPS, VSync, gameplay options, and key bindings.

use crate::keys::{Key, KeyBindings, KeysConfig};
use sdl2::keyboard::Scancode;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const DEFAULT_CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
  /// "Windowed", "Fullscreen", or "Borderless"
  #[serde(default = "default_window_mode")]
  pub window_mode: String,
  /// Window scaling factor (1 for 640x480, 2 for 1280x960, 3 for 1920x1440)
  #[serde(default = "default_scale")]
  pub scale: u32,
  /// Window width (default: 1280, for 2x crisp scaling of 640x480)
  #[serde(default = "default_width")]
  pub width: u32,
  /// Window height (default: 960)
  #[serde(default = "default_height")]
  pub height: u32,
  /// Enable VSync
  #[serde(default = "default_true")]
  pub vsync: bool,
  /// Target FPS limit (60, 120, 144, or 0 for unlimited)
  #[serde(default = "default_fps")]
  pub target_fps: u32,
  /// Keep 4:3 original retro aspect ratio with black letterboxing
  #[serde(default = "default_true")]
  pub keep_aspect_ratio: bool,
}

fn default_window_mode() -> String {
  "Windowed".to_string()
}
fn default_scale() -> u32 {
  2
}
fn default_width() -> u32 {
  1280
}
fn default_height() -> u32 {
  960
}
fn default_true() -> bool {
  true
}
fn default_fps() -> u32 {
  60
}

impl Default for DisplayConfig {
  fn default() -> Self {
    Self {
      window_mode: default_window_mode(),
      scale: default_scale(),
      width: default_width(),
      height: default_height(),
      vsync: true,
      target_fps: 60,
      keep_aspect_ratio: true,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameplayConfig {
  /// Default speed multiplier (1.0 = normal, 0.5 = slow, 1.5 = fast)
  #[serde(default = "default_speed")]
  pub default_speed: f32,
  /// Auto fill remaining player slots with CPU bots in multiplayer
  #[serde(default = "default_true")]
  pub auto_bots: bool,
  /// Default AI Bot difficulty: "Easy", "Medium", or "Hard"
  #[serde(default = "default_bot_difficulty")]
  pub bot_difficulty: String,
}

fn default_speed() -> f32 {
  1.0
}
fn default_bot_difficulty() -> String {
  "Medium".to_string()
}

impl Default for GameplayConfig {
  fn default() -> Self {
    Self {
      default_speed: default_speed(),
      auto_bots: true,
      bot_difficulty: default_bot_difficulty(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerKeysConfig {
  pub left: String,
  pub right: String,
  pub up: String,
  pub down: String,
  pub stop: String,
  pub bomb: String,
  pub choose: String,
  pub remote: String,
}

impl PlayerKeysConfig {
  pub fn to_key_bindings(&self) -> KeyBindings {
    let mut bindings = KeyBindings::default();
    bindings[Key::Left] = parse_scancode(&self.left);
    bindings[Key::Right] = parse_scancode(&self.right);
    bindings[Key::Up] = parse_scancode(&self.up);
    bindings[Key::Down] = parse_scancode(&self.down);
    bindings[Key::Stop] = parse_scancode(&self.stop);
    bindings[Key::Bomb] = parse_scancode(&self.bomb);
    bindings[Key::Choose] = parse_scancode(&self.choose);
    bindings[Key::Remote] = parse_scancode(&self.remote);
    bindings
  }

  pub fn from_key_bindings(bindings: &KeyBindings) -> Self {
    Self {
      left: scancode_name(bindings[Key::Left], "Left"),
      right: scancode_name(bindings[Key::Right], "Right"),
      up: scancode_name(bindings[Key::Up], "Up"),
      down: scancode_name(bindings[Key::Down], "Down"),
      stop: scancode_name(bindings[Key::Stop], "Space"),
      bomb: scancode_name(bindings[Key::Bomb], "Return"),
      choose: scancode_name(bindings[Key::Choose], "RightShift"),
      remote: scancode_name(bindings[Key::Remote], "RightControl"),
    }
  }
}

pub fn default_player1_keys() -> PlayerKeysConfig {
  PlayerKeysConfig {
    left: "Left".to_string(),
    right: "Right".to_string(),
    up: "Up".to_string(),
    down: "Down".to_string(),
    stop: "Space".to_string(),
    bomb: "Return".to_string(),
    choose: "RightShift".to_string(),
    remote: "RightControl".to_string(),
  }
}

pub fn default_player2_keys() -> PlayerKeysConfig {
  PlayerKeysConfig {
    left: "A".to_string(),
    right: "D".to_string(),
    up: "W".to_string(),
    down: "S".to_string(),
    stop: "Q".to_string(),
    bomb: "E".to_string(),
    choose: "Tab".to_string(),
    remote: "LeftControl".to_string(),
  }
}

pub fn default_player3_keys() -> PlayerKeysConfig {
  PlayerKeysConfig {
    left: "J".to_string(),
    right: "L".to_string(),
    up: "I".to_string(),
    down: "K".to_string(),
    stop: "U".to_string(),
    bomb: "O".to_string(),
    choose: "Y".to_string(),
    remote: "H".to_string(),
  }
}

pub fn default_player4_keys() -> PlayerKeysConfig {
  PlayerKeysConfig {
    left: "Keypad 4".to_string(),
    right: "Keypad 6".to_string(),
    up: "Keypad 8".to_string(),
    down: "Keypad 2".to_string(),
    stop: "Keypad 5".to_string(),
    bomb: "Keypad 0".to_string(),
    choose: "Keypad Enter".to_string(),
    remote: "Keypad +".to_string(),
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysSection {
  #[serde(default = "default_player1_keys")]
  pub player1: PlayerKeysConfig,
  #[serde(default = "default_player2_keys")]
  pub player2: PlayerKeysConfig,
  #[serde(default = "default_player3_keys")]
  pub player3: PlayerKeysConfig,
  #[serde(default = "default_player4_keys")]
  pub player4: PlayerKeysConfig,
}

impl Default for KeysSection {
  fn default() -> Self {
    Self {
      player1: default_player1_keys(),
      player2: default_player2_keys(),
      player3: default_player3_keys(),
      player4: default_player4_keys(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
  #[serde(default)]
  pub display: DisplayConfig,
  #[serde(default)]
  pub gameplay: GameplayConfig,
  #[serde(default)]
  pub keys: KeysSection,
}

impl AppConfig {
  /// Load config from file. If file does not exist, creates it with defaults.
  pub fn load_or_create(dir: &Path) -> Self {
    let file = dir.join(DEFAULT_CONFIG_FILE);
    if file.is_file() {
      match std::fs::read_to_string(&file) {
        Ok(content) => match toml::from_str::<AppConfig>(&content) {
          Ok(cfg) => return cfg,
          Err(err) => {
            eprintln!("Warning: Failed to parse config.toml ({}). Using defaults.", err);
          }
        },
        Err(err) => {
          eprintln!("Warning: Failed to read config.toml ({}). Using defaults.", err);
        }
      }
    }

    let default_cfg = AppConfig::default();
    let _ = default_cfg.save(dir);
    default_cfg
  }

  /// Save current configuration to `config.toml`.
  pub fn save(&self, dir: &Path) -> Result<(), anyhow::Error> {
    let file = dir.join(DEFAULT_CONFIG_FILE);
    let content = toml::to_string_pretty(self)?;
    std::fs::write(file, content)?;
    Ok(())
  }

  /// Apply configured keybindings to `KeysConfig`.
  pub fn apply_to_keys_config(&self, keys_config: &mut KeysConfig) {
    keys_config.keys[0] = self.keys.player1.to_key_bindings();
    keys_config.keys[1] = self.keys.player2.to_key_bindings();
    keys_config.keys[2] = self.keys.player3.to_key_bindings();
    keys_config.keys[3] = self.keys.player4.to_key_bindings();
  }

  /// Update `keys` section from `KeysConfig`.
  pub fn update_from_keys_config(&mut self, keys_config: &KeysConfig) {
    self.keys.player1 = PlayerKeysConfig::from_key_bindings(&keys_config.keys[0]);
    self.keys.player2 = PlayerKeysConfig::from_key_bindings(&keys_config.keys[1]);
    self.keys.player3 = PlayerKeysConfig::from_key_bindings(&keys_config.keys[2]);
    self.keys.player4 = PlayerKeysConfig::from_key_bindings(&keys_config.keys[3]);
  }
}

pub fn parse_scancode(name: &str) -> Option<Scancode> {
  let trimmed = name.trim();
  if let Some(scancode) = Scancode::from_name(trimmed) {
    return Some(scancode);
  }
  // Try case-insensitive / friendly aliases
  match trimmed.to_lowercase().as_str() {
    "up" => Some(Scancode::Up),
    "down" => Some(Scancode::Down),
    "left" => Some(Scancode::Left),
    "right" => Some(Scancode::Right),
    "space" => Some(Scancode::Space),
    "enter" | "return" => Some(Scancode::Return),
    "tab" => Some(Scancode::Tab),
    "escape" | "esc" => Some(Scancode::Escape),
    "backspace" => Some(Scancode::Backspace),
    "leftshift" | "lshift" => Some(Scancode::LShift),
    "rightshift" | "rshift" => Some(Scancode::RShift),
    "leftctrl" | "leftcontrol" | "lctrl" => Some(Scancode::LCtrl),
    "rightctrl" | "rightcontrol" | "rctrl" => Some(Scancode::RCtrl),
    "leftalt" | "lalt" => Some(Scancode::LAlt),
    "rightalt" | "ralt" => Some(Scancode::RAlt),
    "a" => Some(Scancode::A),
    "b" => Some(Scancode::B),
    "c" => Some(Scancode::C),
    "d" => Some(Scancode::D),
    "e" => Some(Scancode::E),
    "f" => Some(Scancode::F),
    "g" => Some(Scancode::G),
    "h" => Some(Scancode::H),
    "i" => Some(Scancode::I),
    "j" => Some(Scancode::J),
    "k" => Some(Scancode::K),
    "l" => Some(Scancode::L),
    "m" => Some(Scancode::M),
    "n" => Some(Scancode::N),
    "o" => Some(Scancode::O),
    "p" => Some(Scancode::P),
    "q" => Some(Scancode::Q),
    "r" => Some(Scancode::R),
    "s" => Some(Scancode::S),
    "t" => Some(Scancode::T),
    "u" => Some(Scancode::U),
    "v" => Some(Scancode::V),
    "w" => Some(Scancode::W),
    "x" => Some(Scancode::X),
    "y" => Some(Scancode::Y),
    "z" => Some(Scancode::Z),
    "kp0" | "num0" | "keypad 0" => Some(Scancode::Kp0),
    "kp1" | "num1" | "keypad 1" => Some(Scancode::Kp1),
    "kp2" | "num2" | "keypad 2" => Some(Scancode::Kp2),
    "kp3" | "num3" | "keypad 3" => Some(Scancode::Kp3),
    "kp4" | "num4" | "keypad 4" => Some(Scancode::Kp4),
    "kp5" | "num5" | "keypad 5" => Some(Scancode::Kp5),
    "kp6" | "num6" | "keypad 6" => Some(Scancode::Kp6),
    "kp7" | "num7" | "keypad 7" => Some(Scancode::Kp7),
    "kp8" | "num8" | "keypad 8" => Some(Scancode::Kp8),
    "kp9" | "num9" | "keypad 9" => Some(Scancode::Kp9),
    "kp_enter" | "keypad enter" => Some(Scancode::KpEnter),
    "kp_plus" | "keypad +" => Some(Scancode::KpPlus),
    "kp_minus" | "keypad -" => Some(Scancode::KpMinus),
    _ => None,
  }
}

pub fn scancode_name(code: Option<Scancode>, fallback: &str) -> String {
  code.map(|c| c.name().to_string()).unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_default_config_toml_roundtrip() {
    let cfg = AppConfig::default();
    let toml_str = toml::to_string_pretty(&cfg).expect("Failed to serialize");
    assert!(toml_str.contains("[display]"));
    assert!(toml_str.contains("width = 1280"));
    assert!(toml_str.contains("height = 960"));
    assert!(toml_str.contains("vsync = true"));
    assert!(toml_str.contains("target_fps = 60"));
    assert!(toml_str.contains("keep_aspect_ratio = true"));
    assert!(toml_str.contains("[gameplay]"));
    assert!(toml_str.contains("default_speed = 1.0"));
    assert!(toml_str.contains("[keys.player1]"));

    let deserialized: AppConfig = toml::from_str(&toml_str).expect("Failed to deserialize");
    assert_eq!(deserialized.display.width, 1280);
    assert_eq!(deserialized.display.scale, 2);
    assert_eq!(deserialized.display.target_fps, 60);
    assert!(deserialized.display.keep_aspect_ratio);
    assert_eq!(deserialized.gameplay.bot_difficulty, "Medium");
    assert_eq!(deserialized.keys.player1.left, "Left");
    assert_eq!(deserialized.keys.player2.left, "A");
  }

  #[test]
  fn test_parse_scancodes() {
    assert_eq!(parse_scancode("Up"), Some(Scancode::Up));
    assert_eq!(parse_scancode("left"), Some(Scancode::Left));
    assert_eq!(parse_scancode("Space"), Some(Scancode::Space));
    assert_eq!(parse_scancode("Enter"), Some(Scancode::Return));
    assert_eq!(parse_scancode("w"), Some(Scancode::W));
    assert_eq!(parse_scancode("Keypad 0"), Some(Scancode::Kp0));
    assert_eq!(parse_scancode("invalid_unknown_key"), None);
  }

  #[test]
  fn test_custom_config_parsing() {
    let toml_content = r#"
[display]
window_mode = "Fullscreen"
scale = 3
width = 1920
height = 1440
vsync = false
target_fps = 144
keep_aspect_ratio = false

[gameplay]
default_speed = 1.5
auto_bots = false
bot_difficulty = "Hard"

[keys.player1]
left = "A"
right = "D"
up = "W"
down = "S"
stop = "Space"
bomb = "J"
choose = "K"
remote = "L"
"#;
    let cfg: AppConfig = toml::from_str(toml_content).expect("Failed to parse custom toml");
    assert_eq!(cfg.display.window_mode, "Fullscreen");
    assert_eq!(cfg.display.scale, 3);
    assert_eq!(cfg.display.width, 1920);
    assert_eq!(cfg.display.target_fps, 144);
    assert!(!cfg.display.vsync);
    assert!(!cfg.display.keep_aspect_ratio);
    assert_eq!(cfg.gameplay.default_speed, 1.5);
    assert!(!cfg.gameplay.auto_bots);
    assert_eq!(cfg.gameplay.bot_difficulty, "Hard");
    assert_eq!(cfg.keys.player1.left, "A");
    assert_eq!(cfg.keys.player1.bomb, "J");
  }
}
