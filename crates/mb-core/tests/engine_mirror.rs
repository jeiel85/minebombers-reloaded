//! `crates/mb-core/src/world/` is a copy of the desktop crate's `src/world/`, because the web
//! edition (`crates/mb-wasm`) needs the engine without SDL2. Nothing enforced that, and the copy had
//! already drifted once: the fixes from #17 (don't fire a grenade into the wall next to you, don't
//! pick a bomb whose blast a one-tile retreat cannot clear) landed in `src/world/bot.rs` only, so
//! the web edition kept shipping the bug for two releases.
//!
//! This compares the two directories file by file. `bot.rs` is compared up to its test module only,
//! since the desktop copy's tests use `menu::shop` and the game's data files, neither of which exist
//! in this crate.

use std::path::{Path, PathBuf};

fn desktop_world() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/world")
}

fn core_world() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("src/world")
}

/// Line endings normalized, so that a checkout with CRLF endings does not read as a difference.
fn normalized(source: &str) -> String {
  source.replace("\r\n", "\n")
}

/// Everything before the unit tests, which are allowed to differ.
fn implementation(source: &str) -> String {
  let source = match source.find("#[cfg(test)]") {
    Some(at) => &source[..at],
    None => source,
  };
  normalized(source)
}

#[test]
fn mb_core_world_mirrors_the_desktop_engine() {
  let mut mismatched = Vec::new();
  let mut checked = 0;

  for entry in std::fs::read_dir(core_world()).expect("mb-core world directory") {
    let entry = entry.expect("directory entry");
    let path = entry.path();
    let name = entry.file_name().to_string_lossy().into_owned();

    let counterpart = desktop_world().join(&name);
    if path.is_dir() {
      for sub in std::fs::read_dir(&path).expect("subdirectory") {
        let sub = sub.expect("directory entry");
        let sub_name = sub.file_name().to_string_lossy().into_owned();
        let core_source = std::fs::read_to_string(sub.path()).expect("readable source file");
        let desktop_path = counterpart.join(&sub_name);
        let Ok(desktop_source) = std::fs::read_to_string(&desktop_path) else {
          mismatched.push(format!("{}/{} has no counterpart in src/world", name, sub_name));
          continue;
        };
        checked += 1;
        if normalized(&core_source) != normalized(&desktop_source) {
          mismatched.push(format!(
            "{}/{} differs from src/world/{}/{}",
            name, sub_name, name, sub_name
          ));
        }
      }
      continue;
    }

    let core_source = std::fs::read_to_string(&path).expect("readable source file");
    let Ok(desktop_source) = std::fs::read_to_string(&counterpart) else {
      mismatched.push(format!("{} has no counterpart in src/world", name));
      continue;
    };
    checked += 1;
    if implementation(&core_source) != implementation(&desktop_source) {
      mismatched.push(format!("{} differs from src/world/{}", name, name));
    }
  }

  assert!(checked > 5, "only {} files compared - did the layout move?", checked);
  assert!(
    mismatched.is_empty(),
    "the web edition's engine copy has drifted from the desktop one; copy the files over:\n  {}",
    mismatched.join("\n  ")
  );
}
