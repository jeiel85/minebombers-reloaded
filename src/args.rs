use crate::gamedir::{self, Inputs, Located, Ui, GAME_DIR_ENV};
use crate::userdata::{self, Platform};
use std::path::PathBuf;

pub struct Args {
  pub path: PathBuf,
  pub campaign_mode: bool,
}

const DIALOG_TITLE: &str = "Mine Bombers: game files needed";

/// A message dialog. Windows gets the standard message box, which lays long text out correctly. Elsewhere
/// SDL's message box is used; on Windows SDL's box sizes itself so badly that it cuts off the last lines.
#[cfg(windows)]
fn message_dialog(is_error: bool, text: &str) {
  let level = if is_error { rfd::MessageLevel::Error } else { rfd::MessageLevel::Info };
  let _ = rfd::MessageDialog::new()
    .set_level(level)
    .set_title(DIALOG_TITLE)
    .set_description(text)
    .set_buttons(rfd::MessageButtons::Ok)
    .show();
}

#[cfg(not(windows))]
fn message_dialog(is_error: bool, text: &str) {
  use sdl2::messagebox::{show_simple_message_box, MessageBoxFlag};
  let flag = if is_error { MessageBoxFlag::ERROR } else { MessageBoxFlag::INFORMATION };
  let _ = show_simple_message_box(flag, DIALOG_TITLE, text, None);
}

/// Native dialogs. The program is a Windows GUI executable, so nobody would see anything printed to
/// stderr; questions and errors go through dialogs instead.
struct NativeUi;

impl Ui for NativeUi {
  fn pick_folder(&self, reason: &str) -> Option<PathBuf> {
    message_dialog(false, reason);
    rfd::FileDialog::new()
      .set_title("Select your Mine Bombers 3.11 folder")
      .pick_folder()
  }

  fn show_error(&self, message: &str) {
    eprintln!("{}", message);
    message_dialog(true, message);
  }
}

pub fn parse_args() -> Args {
  let mut explicit = None;
  let mut campaign_mode = false;
  let mut force_choose = false;
  for arg in std::env::args().skip(1) {
    match arg.as_str() {
      "--campaign" => campaign_mode = true,
      "--choose-game-folder" => force_choose = true,
      "--help" => {
        eprintln!("Mine Bombers 3.11 (Native PC Engine)\n");
        eprintln!("USAGE:");
        eprintln!("    MineBombers [--campaign] [--choose-game-folder] [game-folder]\n");
        eprintln!("The game files of the original Mine Bombers 3.11 (freeware) are not included.");
        eprintln!("On the first run a window asks for the folder that has them, and the choice is");
        eprintln!("remembered. Use --choose-game-folder to pick another folder later, or pass the");
        eprintln!("folder on the command line or in {} to override it once.", GAME_DIR_ENV);
        std::process::exit(0);
      }
      arg => explicit = Some(PathBuf::from(arg)),
    }
  }

  let cur = std::env::current_dir().unwrap_or_default();
  let exe_dir = std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    .unwrap_or_default();
  let inputs = Inputs {
    explicit,
    from_env: std::env::var_os(GAME_DIR_ENV).filter(|v| !v.is_empty()).map(PathBuf::from),
    nearby: vec![
      cur.join("res").join("minebomb"),
      exe_dir.join("res").join("minebomb"),
      cur.clone(),
      exe_dir,
    ],
    force_choose,
  };
  let user_dir = userdata::user_dir_for(Platform::current(), |name| std::env::var_os(name));

  match gamedir::locate(&inputs, user_dir.as_deref(), &NativeUi) {
    Located::Found(path) => Args { path, campaign_mode },
    Located::Cancelled => std::process::exit(0),
    Located::Invalid(message) => {
      NativeUi.show_error(&message);
      std::process::exit(1)
    }
  }
}
