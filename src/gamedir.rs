//! Finding the user's copy of the original game.
//!
//! This works the way OpenRCT2 finds RollerCoaster Tycoon 2. The game files are not part of this program,
//! so on the first run a folder chooser opens and the user picks the folder with their Mine Bombers 3.11
//! files. The choice is checked and remembered, so it is asked only once. Anything more explicit wins over
//! the remembered folder: a folder on the command line, or the `MINEBOMBERS_GAME_DIR` environment variable.

use std::path::{Path, PathBuf};

/// A file every Mine Bombers 3.11 folder has; its presence decides whether a folder is the game.
const MARKER: &str = "TITLEBE.SPY";
/// Where the chosen folder is remembered, inside the per-user folder.
const REMEMBERED_FILE: &str = "game_path.txt";

pub const GAME_DIR_ENV: &str = "MINEBOMBERS_GAME_DIR";

pub fn is_game_dir(dir: &Path) -> bool {
  dir.join(MARKER).is_file()
}

/// The folder chosen on an earlier run, if one was remembered.
pub fn remembered(user_dir: &Path) -> Option<PathBuf> {
  let text = std::fs::read_to_string(user_dir.join(REMEMBERED_FILE)).ok()?;
  let line = text.lines().next()?.trim();
  if line.is_empty() {
    None
  } else {
    Some(PathBuf::from(line))
  }
}

pub fn remember(user_dir: &Path, game_dir: &Path) -> std::io::Result<()> {
  let text = game_dir
    .to_str()
    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "the folder name is not valid text"))?;
  std::fs::create_dir_all(user_dir)?;
  std::fs::write(user_dir.join(REMEMBERED_FILE), format!("{}\n", text))
}

/// How the user is asked. The program uses native dialogs; tests use a stand-in.
pub trait Ui {
  /// Explains why the game folder is needed and lets the user pick one. `None` means they cancelled.
  fn pick_folder(&self, reason: &str) -> Option<PathBuf>;
  fn show_error(&self, message: &str);
}

/// What the caller already knows about where the game is.
#[derive(Default)]
pub struct Inputs {
  /// A folder given on the command line
  pub explicit: Option<PathBuf>,
  /// The folder from `MINEBOMBERS_GAME_DIR`
  pub from_env: Option<PathBuf>,
  /// Folders next to the program that are worth trying, in order
  pub nearby: Vec<PathBuf>,
  /// Ignore everything and ask, to change the remembered folder
  pub force_choose: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Located {
  Found(PathBuf),
  /// The user closed the folder chooser
  Cancelled,
  /// A folder given explicitly is not the game; the message says which one
  Invalid(String),
}

const FIRST_RUN: &str = "Mine Bombers needs the game files of
the original Mine Bombers 3.11
(freeware). They are not included
with this program.

Download the game, for example from
https://archive.org/details/mnb311fw
and unpack it.

Then click OK and choose the folder
with the game files. You only need
to do this once.";

/// Finds the game folder, asking the user if nothing else says where it is. A folder the user picks is
/// remembered in `user_dir` (when there is one).
pub fn locate(inputs: &Inputs, user_dir: Option<&Path>, ui: &dyn Ui) -> Located {
  let mut reason = FIRST_RUN.to_string();

  if !inputs.force_choose {
    for (what, path) in [
      ("The folder given on the command line", &inputs.explicit),
      (GAME_DIR_ENV, &inputs.from_env),
    ] {
      if let Some(path) = path {
        return if is_game_dir(path) {
          Located::Found(path.clone())
        } else {
          Located::Invalid(format!(
            "{} ('{}') does not contain Mine Bombers 3.11: {} is missing.",
            what,
            path.display(),
            MARKER
          ))
        };
      }
    }

    if let Some(saved) = user_dir.and_then(remembered) {
      if is_game_dir(&saved) {
        return Located::Found(saved);
      }
      reason = format!(
        "The folder you chose earlier ('{}') no longer contains the game files.\n\n{}",
        saved.display(),
        FIRST_RUN
      );
    }

    if let Some(nearby) = inputs.nearby.iter().find(|dir| is_game_dir(dir)) {
      return Located::Found(nearby.clone());
    }
  }

  loop {
    let Some(dir) = ui.pick_folder(&reason) else {
      return Located::Cancelled;
    };
    if is_game_dir(&dir) {
      if let Some(user_dir) = user_dir {
        if let Err(err) = remember(user_dir, &dir) {
          eprintln!("Warning: could not remember the game folder ({}).", err);
        }
      }
      return Located::Found(dir);
    }
    ui.show_error(&format!(
      "'{}' does not contain Mine Bombers 3.11: {} was not found.\n\nChoose the folder that holds the game files.",
      dir.display(),
      MARKER
    ));
    reason = "Choose the folder that holds the Mine Bombers 3.11 game files.".to_string();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::cell::RefCell;
  use std::collections::VecDeque;

  struct FakeUi {
    picks: RefCell<VecDeque<Option<PathBuf>>>,
    reasons: RefCell<Vec<String>>,
    errors: RefCell<Vec<String>>,
  }

  impl FakeUi {
    fn new(picks: Vec<Option<PathBuf>>) -> Self {
      FakeUi {
        picks: RefCell::new(picks.into()),
        reasons: RefCell::new(Vec::new()),
        errors: RefCell::new(Vec::new()),
      }
    }
    fn asked(&self) -> usize {
      self.reasons.borrow().len()
    }
  }

  impl Ui for FakeUi {
    fn pick_folder(&self, reason: &str) -> Option<PathBuf> {
      self.reasons.borrow_mut().push(reason.to_string());
      self.picks.borrow_mut().pop_front().expect("asked more often than the test expected")
    }
    fn show_error(&self, message: &str) {
      self.errors.borrow_mut().push(message.to_string());
    }
  }

  fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mb-gamedir-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
  }

  fn game(name: &str) -> PathBuf {
    let dir = temp(name);
    std::fs::write(dir.join(MARKER), b"x").unwrap();
    dir
  }

  #[test]
  fn a_valid_command_line_folder_is_used_without_asking() {
    let (dir, ui) = (game("cli"), FakeUi::new(vec![]));
    let inputs = Inputs { explicit: Some(dir.clone()), ..Default::default() };
    assert_eq!(locate(&inputs, None, &ui), Located::Found(dir));
    assert_eq!(ui.asked(), 0);
  }

  #[test]
  fn a_wrong_command_line_folder_is_an_error_not_a_fallback() {
    let (wrong, other, ui) = (temp("wrong"), game("other"), FakeUi::new(vec![]));
    let inputs = Inputs { explicit: Some(wrong), nearby: vec![other], ..Default::default() };
    assert!(matches!(locate(&inputs, None, &ui), Located::Invalid(m) if m.contains("command line") && m.contains(MARKER)));
    assert_eq!(ui.asked(), 0);
  }

  #[test]
  fn the_environment_variable_is_used_when_there_is_no_command_line_folder() {
    let (dir, ui) = (game("env"), FakeUi::new(vec![]));
    let inputs = Inputs { from_env: Some(dir.clone()), ..Default::default() };
    assert_eq!(locate(&inputs, None, &ui), Located::Found(dir));
    let bad = Inputs { from_env: Some(temp("envbad")), ..Default::default() };
    assert!(matches!(locate(&bad, None, &ui), Located::Invalid(m) if m.contains(GAME_DIR_ENV)));
  }

  #[test]
  fn the_remembered_folder_is_used_without_asking() {
    let (user, dir, ui) = (temp("user1"), game("saved"), FakeUi::new(vec![]));
    remember(&user, &dir).unwrap();
    assert_eq!(locate(&Inputs::default(), Some(&user), &ui), Located::Found(dir));
    assert_eq!(ui.asked(), 0);
  }

  #[test]
  fn a_remembered_folder_that_is_gone_makes_it_ask_again_and_says_why() {
    let (user, gone, fresh) = (temp("user2"), temp("gone"), game("fresh"));
    remember(&user, &gone).unwrap();
    let ui = FakeUi::new(vec![Some(fresh.clone())]);
    assert_eq!(locate(&Inputs::default(), Some(&user), &ui), Located::Found(fresh.clone()));
    assert!(ui.reasons.borrow()[0].contains("no longer contains"));
    assert_eq!(remembered(&user), Some(fresh), "the new choice replaces the old one");
  }

  #[test]
  fn a_nearby_folder_is_used_before_asking() {
    let (near, ui) = (game("near"), FakeUi::new(vec![]));
    let inputs = Inputs { nearby: vec![temp("empty"), near.clone()], ..Default::default() };
    assert_eq!(locate(&inputs, None, &ui), Located::Found(near));
    assert_eq!(ui.asked(), 0);
  }

  #[test]
  fn with_nothing_known_it_asks_and_remembers_the_choice() {
    let (user, dir) = (temp("user3"), game("picked"));
    let ui = FakeUi::new(vec![Some(dir.clone())]);
    assert_eq!(locate(&Inputs::default(), Some(&user), &ui), Located::Found(dir.clone()));
    assert!(ui.reasons.borrow()[0].contains("archive.org"), "the first question says where to get the game");
    assert_eq!(remembered(&user), Some(dir.clone()));
    // the next run needs no question
    let quiet = FakeUi::new(vec![]);
    assert_eq!(locate(&Inputs::default(), Some(&user), &quiet), Located::Found(dir));
  }

  #[test]
  fn a_wrong_choice_is_explained_and_asked_again() {
    let (user, wrong, right) = (temp("user4"), temp("notgame"), game("game"));
    let ui = FakeUi::new(vec![Some(wrong.clone()), Some(right.clone())]);
    assert_eq!(locate(&Inputs::default(), Some(&user), &ui), Located::Found(right));
    assert_eq!(ui.errors.borrow().len(), 1);
    assert!(ui.errors.borrow()[0].contains(&*wrong.display().to_string()));
    assert_eq!(ui.asked(), 2);
  }

  #[test]
  fn cancelling_gives_up_and_remembers_nothing() {
    let user = temp("user5");
    let ui = FakeUi::new(vec![None]);
    assert_eq!(locate(&Inputs::default(), Some(&user), &ui), Located::Cancelled);
    assert_eq!(remembered(&user), None);
  }

  #[test]
  fn choosing_again_ignores_everything_that_is_known() {
    let (user, known, chosen) = (temp("user6"), game("known"), game("chosen"));
    remember(&user, &known).unwrap();
    let inputs = Inputs { explicit: Some(known.clone()), from_env: Some(known.clone()), nearby: vec![known], force_choose: true };
    let ui = FakeUi::new(vec![Some(chosen.clone())]);
    assert_eq!(locate(&inputs, Some(&user), &ui), Located::Found(chosen.clone()));
    assert_eq!(remembered(&user), Some(chosen));
  }

  #[test]
  fn without_a_user_folder_it_still_works_but_cannot_remember() {
    let (dir, ui) = (game("nouser"), FakeUi::new(vec![Some(game("nouser"))]));
    assert_eq!(locate(&Inputs::default(), None, &ui), Located::Found(dir));
  }

  #[test]
  fn an_empty_or_missing_memory_file_means_nothing_is_remembered() {
    let user = temp("user7");
    assert_eq!(remembered(&user), None);
    std::fs::write(user.join(REMEMBERED_FILE), "\n").unwrap();
    assert_eq!(remembered(&user), None);
  }
}
