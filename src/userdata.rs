//! Where the game keeps the files it writes.
//!
//! The game folder is the user's copy of the original game, so the engine only reads it. Everything the
//! engine writes (settings, key bindings, options, high scores, player statistics) goes to a per-user
//! folder instead. On the first run, files that the original game (or an earlier version of this engine)
//! had already written into the game folder are copied over, so records carry on while the originals stay
//! untouched. The original DOS game therefore never sees anything this engine writes, and a game folder
//! in a read-only location works.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The files the engine writes. Everything else in the game folder is read-only game data.
pub const USER_FILES: [&str; 6] = [
  "config.toml",
  "keysrel.cfg",
  "OPTIONS.CFG",
  "HIGHSCOR.DAT",
  "PLAYERS.DAT",
  "IDENTIFY.DAT",
];

/// Overrides the per-user folder (portable installs, tests).
pub const USER_DIR_ENV: &str = "MINEBOMBERS_USER_DIR";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
  Windows,
  MacOs,
  Other,
}

impl Platform {
  pub fn current() -> Self {
    if cfg!(target_os = "windows") {
      Platform::Windows
    } else if cfg!(target_os = "macos") {
      Platform::MacOs
    } else {
      Platform::Other
    }
  }
}

/// The per-user folder for `platform`, given a way to read environment variables.
pub fn user_dir_for(platform: Platform, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
  if let Some(dir) = env(USER_DIR_ENV).filter(|d| !d.is_empty()) {
    return Some(PathBuf::from(dir));
  }
  let non_empty = |name: &str| env(name).filter(|v| !v.is_empty()).map(PathBuf::from);
  match platform {
    Platform::Windows => non_empty("APPDATA").map(|dir| dir.join("MineBombers")),
    Platform::MacOs => non_empty("HOME").map(|dir| dir.join("Library").join("Application Support").join("MineBombers")),
    Platform::Other => non_empty("XDG_CONFIG_HOME")
      .or_else(|| non_empty("HOME").map(|home| home.join(".config")))
      .map(|dir| dir.join("minebombers")),
  }
}

/// Copies the engine's files that exist in `game_dir` but not yet in `user_dir`, and returns their names.
/// Nothing is ever written to, changed in or removed from `game_dir`.
pub fn migrate(game_dir: &Path, user_dir: &Path) -> Vec<&'static str> {
  let mut copied = Vec::new();
  for name in USER_FILES {
    let (from, to) = (game_dir.join(name), user_dir.join(name));
    if from.is_file() && !to.exists() && std::fs::copy(&from, &to).is_ok() {
      copied.push(name);
    }
  }
  copied
}

/// The folder for the files the engine writes: created if needed, with existing files imported from the
/// game folder. If no per-user folder can be made, warns and falls back to the game folder.
pub fn prepare(game_dir: &Path) -> PathBuf {
  let Some(dir) = user_dir_for(Platform::current(), |name| std::env::var_os(name)) else {
    eprintln!("Warning: no per-user folder is available; saving next to the game files instead.");
    return game_dir.to_path_buf();
  };
  if let Err(err) = std::fs::create_dir_all(&dir) {
    eprintln!("Warning: cannot create '{}' ({}); saving next to the game files instead.", dir.display(), err);
    return game_dir.to_path_buf();
  }
  let copied = migrate(game_dir, &dir);
  if !copied.is_empty() {
    eprintln!("Copied {} from the game folder to '{}'.", copied.join(", "), dir.display());
  }
  dir
}

#[cfg(test)]
mod tests {
  use super::*;

  fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| pairs.iter().find(|(n, _)| *n == name).map(|(_, v)| OsString::from(*v))
  }

  fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mb-userdata-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
  }

  #[test]
  fn windows_uses_appdata() {
    let dir = user_dir_for(Platform::Windows, env(&[("APPDATA", "C:/Users/me/AppData/Roaming")]));
    assert_eq!(dir, Some(PathBuf::from("C:/Users/me/AppData/Roaming").join("MineBombers")));
  }

  #[test]
  fn linux_prefers_xdg_config_home_then_home() {
    let xdg = user_dir_for(Platform::Other, env(&[("XDG_CONFIG_HOME", "/x"), ("HOME", "/home/me")]));
    assert_eq!(xdg, Some(PathBuf::from("/x").join("minebombers")));
    let home = user_dir_for(Platform::Other, env(&[("HOME", "/home/me")]));
    assert_eq!(home, Some(PathBuf::from("/home/me").join(".config").join("minebombers")));
    // an empty XDG_CONFIG_HOME counts as unset, as the XDG specification says
    let empty = user_dir_for(Platform::Other, env(&[("XDG_CONFIG_HOME", ""), ("HOME", "/home/me")]));
    assert_eq!(empty, home);
  }

  #[test]
  fn macos_uses_application_support() {
    let dir = user_dir_for(Platform::MacOs, env(&[("HOME", "/Users/me")]));
    assert_eq!(dir, Some(PathBuf::from("/Users/me/Library/Application Support/MineBombers")));
  }

  #[test]
  fn the_override_wins_on_every_platform() {
    for platform in [Platform::Windows, Platform::MacOs, Platform::Other] {
      let dir = user_dir_for(platform, env(&[(USER_DIR_ENV, "/portable"), ("APPDATA", "/a"), ("HOME", "/h")]));
      assert_eq!(dir, Some(PathBuf::from("/portable")));
    }
  }

  #[test]
  fn no_usable_environment_gives_none() {
    for platform in [Platform::Windows, Platform::MacOs, Platform::Other] {
      assert_eq!(user_dir_for(platform, env(&[])), None);
    }
    assert_eq!(user_dir_for(Platform::Windows, env(&[("APPDATA", "")])), None);
  }

  #[test]
  fn migrate_imports_only_missing_engine_files_and_never_touches_the_game_folder() {
    let game = temp("game");
    let user = temp("user");
    std::fs::write(game.join("HIGHSCOR.DAT"), b"scores from the original").unwrap();
    std::fs::write(game.join("PLAYERS.DAT"), b"players from the original").unwrap();
    std::fs::write(game.join("TITLEBE.SPY"), b"game data, not ours to copy").unwrap();
    std::fs::write(user.join("PLAYERS.DAT"), b"newer players").unwrap();

    let before: Vec<_> = ["HIGHSCOR.DAT", "PLAYERS.DAT", "TITLEBE.SPY"]
      .iter()
      .map(|n| std::fs::read(game.join(n)).unwrap())
      .collect();
    let copied = migrate(&game, &user);

    assert_eq!(copied, vec!["HIGHSCOR.DAT"], "only the missing engine file is copied");
    assert_eq!(std::fs::read(user.join("HIGHSCOR.DAT")).unwrap(), b"scores from the original");
    assert_eq!(std::fs::read(user.join("PLAYERS.DAT")).unwrap(), b"newer players", "existing files are kept");
    assert!(!user.join("TITLEBE.SPY").exists(), "game data is never copied");
    let after: Vec<_> = ["HIGHSCOR.DAT", "PLAYERS.DAT", "TITLEBE.SPY"]
      .iter()
      .map(|n| std::fs::read(game.join(n)).unwrap())
      .collect();
    assert_eq!(before, after, "the game folder is left exactly as it was");
    assert_eq!(std::fs::read_dir(&game).unwrap().count(), 3, "and nothing was added to it");

    // running it again changes nothing
    assert!(migrate(&game, &user).is_empty());
    let _ = std::fs::remove_dir_all(&game);
    let _ = std::fs::remove_dir_all(&user);
  }

  #[test]
  fn migrating_a_folder_onto_itself_is_a_no_op() {
    let dir = temp("same");
    std::fs::write(dir.join("config.toml"), b"x").unwrap();
    assert!(migrate(&dir, &dir).is_empty());
    assert_eq!(std::fs::read(dir.join("config.toml")).unwrap(), b"x");
    let _ = std::fs::remove_dir_all(&dir);
  }
}
