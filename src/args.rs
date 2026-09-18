use std::path::PathBuf;

pub struct Args {
  pub path: PathBuf,
  pub campaign_mode: bool,
}

pub fn parse_args() -> Args {
  let mut args = Args {
    path: Default::default(),
    campaign_mode: false,
  };
  for arg in std::env::args().skip(1) {
    match arg.as_str() {
      "--campaign" => {
        args.campaign_mode = true;
      }
      "--help" => {
        eprintln!("Mine Bombers 3.11 (Native PC Engine)\n");
        eprintln!("USAGE:");
        eprintln!("    MineBombers [--campaign] [game-path]");
        std::process::exit(0);
      }
      arg => {
        args.path = PathBuf::from(arg);
      }
    }
  }
  if args.path.as_os_str().is_empty() || !args.path.join("TITLEBE.SPY").is_file() {
    let cur = std::env::current_dir().unwrap_or_default();
    let exe_dir = std::env::current_exe()
      .ok()
      .and_then(|p| p.parent().map(|p| p.to_path_buf()))
      .unwrap_or_default();

    if cur.join("res").join("minebomb").join("TITLEBE.SPY").is_file() {
      args.path = cur.join("res").join("minebomb");
    } else if exe_dir.join("res").join("minebomb").join("TITLEBE.SPY").is_file() {
      args.path = exe_dir.join("res").join("minebomb");
    } else if std::path::Path::new("res/minebomb/TITLEBE.SPY").is_file() {
      args.path = PathBuf::from("res/minebomb");
    } else if cur.join("TITLEBE.SPY").is_file() {
      args.path = cur;
    } else if exe_dir.join("TITLEBE.SPY").is_file() {
      args.path = exe_dir;
    }
  }

  if !args.path.is_dir() || !args.path.join("TITLEBE.SPY").is_file() {
    eprintln!(
      "'{}' is not a valid game directory (must be a directory with 'TITLEBE.SPY' file).",
      args.path.display()
    );
    std::process::exit(1);
  }
  args
}
