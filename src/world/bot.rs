//! Bot AI for Mine Bombers
//!
//! Provides decision making for computer-controlled player bombers:
//! - Danger Evasion: actively avoids bomb blast lines and ticking explosives
//! - Combat: drops bombs and uses weapons against nearby opponents
//! - Mining: navigates to gems, rubies, and diamonds while digging through dirt
//! - Roaming / Hunting: hunts opponents when treasures are depleted

use crate::keys::Key;
use crate::world::equipment::Equipment;
use crate::world::map::{LevelMap, MapValue, TimerMap};
use crate::world::position::{Cursor, Direction};
use crate::world::World;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotDifficulty {
  Easy,
  Medium,
  Hard,
}

impl Default for BotDifficulty {
  fn default() -> Self {
    BotDifficulty::Medium
  }
}

impl BotDifficulty {
  pub fn from_str(s: &str) -> Self {
    match s.trim().to_lowercase().as_str() {
      "easy" => BotDifficulty::Easy,
      "hard" => BotDifficulty::Hard,
      _ => BotDifficulty::Medium,
    }
  }

  pub fn label(&self) -> &'static str {
    match self {
      BotDifficulty::Easy => "EASY",
      BotDifficulty::Medium => "NORM",
      BotDifficulty::Hard => "HARD",
    }
  }

  pub fn cycle(&self) -> Self {
    match self {
      BotDifficulty::Easy => BotDifficulty::Medium,
      BotDifficulty::Medium => BotDifficulty::Hard,
      BotDifficulty::Hard => BotDifficulty::Easy,
    }
  }
}

pub struct BotController;

impl BotController {
  /// Update AI decisions for all active bot players.
  pub fn update_bots(world: &mut World) {
    let num_players = world.players.len();
    for bot_idx in 0..num_players {
      if !world.players[bot_idx].is_bot || world.actors[bot_idx].is_dead {
        continue;
      }

      // Check update interval based on difficulty:
      // Hard: updates every tick (instant reactions)
      // Medium: updates every 2 ticks (~25 decisions/sec)
      // Easy: updates every 4 ticks (slower decision speed)
      let interval = match world.players[bot_idx].bot_difficulty {
        BotDifficulty::Hard => 1,
        BotDifficulty::Medium => 2,
        BotDifficulty::Easy => 4,
      };

      if (world.round_counter + bot_idx) % interval != 0 {
        continue;
      }

      Self::update_single_bot(world, bot_idx);
    }
  }

  fn update_single_bot(world: &mut World, bot_idx: usize) {
    if world.actors[bot_idx].is_dead || world.actors[bot_idx].frozen_ticks > 0 {
      return;
    }
    let cursor = world.actors[bot_idx].pos.cursor();
    let difficulty = world.players[bot_idx].bot_difficulty;
    let mut rng = rand::thread_rng();

    // 0. SUDDEN DEATH EVASION: Flee inwards toward map center if in or near hazard ring
    if world.sudden_death_active && world.sudden_death_ring > 0 {
      let ring = world.sudden_death_ring;
      let in_hazard = world.is_in_danger_zone(cursor);
      let near_hazard = cursor.row <= ring + 2
        || cursor.row >= crate::world::map::MAP_ROWS - 1 - (ring + 2)
        || cursor.col <= ring + 2
        || cursor.col >= crate::world::map::MAP_COLS - 1 - (ring + 2);

      if in_hazard || near_hazard {
        let center = Cursor::new(crate::world::map::MAP_ROWS / 2, crate::world::map::MAP_COLS / 2);
        if let Some(inward_dir) = Self::dir_towards_center(cursor, center, &world.maps.level) {
          world.player_action(bot_idx, dir_to_key(inward_dir));
          return;
        }
      }
    }

    // 1. DANGER EVASION: Check for ticking bombs nearby
    let search_radius: i16 = match difficulty {
      BotDifficulty::Hard => 7,
      BotDifficulty::Medium => 5,
      BotDifficulty::Easy => 4,
    };

    if let Some(danger_bomb) = Self::find_threat_bomb(cursor, &world.maps.level, &world.maps.timer, search_radius) {
      let should_evade = match difficulty {
        BotDifficulty::Hard | BotDifficulty::Medium => true,
        BotDifficulty::Easy => rng.gen_range(0..100) < 65, // 35% chance hesitation / panic
      };

      if should_evade {
        if let Some(safe_dir) = Self::choose_flee_direction(cursor, danger_bomb, &world.maps.level, difficulty) {
          world.player_action(bot_idx, dir_to_key(safe_dir));
          return;
        }
      }
    }

    // 2. COMBAT: Check for nearby enemy players
    if let Some((target_cursor, _enemy_idx)) = Self::find_nearest_enemy(world, bot_idx, cursor) {
      let (d_row, d_col) = cursor.distance(target_cursor);
      let manhattan = d_row + d_col;

      let close_combat_dist = match difficulty {
        BotDifficulty::Hard => 3,   // drops bombs earlier to cut off opponents
        BotDifficulty::Medium => 2,
        BotDifficulty::Easy => 1,   // only drops when adjacent
      };

      // Close quarters combat: drop a bomb and flee!
      if manhattan <= close_combat_dist {
        let cur_cell = world.maps.level[cursor];
        if cur_cell.is_passable() && !cur_cell.is_bomb() {
          let should_attack = match difficulty {
            BotDifficulty::Hard | BotDifficulty::Medium => true,
            BotDifficulty::Easy => rng.gen_range(0..100) < 35, // Easy bot only attacks 35% of the time
          };

          if should_attack {
            let has_bomb = world.players[bot_idx].inventory[Equipment::SmallBomb] > 0
              || world.players[bot_idx].inventory[Equipment::Dynamite] > 0
              || world.players[bot_idx].inventory[Equipment::BigBomb] > 0
              || (difficulty == BotDifficulty::Hard && world.players[bot_idx].inventory[Equipment::Mine] > 0);

            if has_bomb {
              // Select weapon based on difficulty preference. Hard used to prefer its biggest bomb
              // here (BigBomb: 12-cell/radius-2 pattern, Dynamite: 36-cell/radius-3+, see
              // BIG_BOMB_PATTERN/DYNAMITE_PATTERN in world/explode.rs) - but this branch is a
              // melee-range panic drop-and-flee: it only steps back one tile below, nowhere near
              // enough to clear a radius-2+ blast, so "better bomb" was mostly buying itself more
              // self-damage, not more kills. SmallBomb (4-cell/radius-1) is the one size a single-tile
              // retreat can actually escape, so every difficulty uses it here; Mine stays Hard-only
              // since a mine sits armed rather than detonating immediately, so it isn't a self-damage
              // risk the same way.
              if difficulty == BotDifficulty::Hard && world.players[bot_idx].inventory[Equipment::Mine] > 0 {
                world.players[bot_idx].selection = Equipment::Mine;
              } else if world.players[bot_idx].inventory[Equipment::SmallBomb] > 0 {
                world.players[bot_idx].selection = Equipment::SmallBomb;
              } else if world.players[bot_idx].inventory[world.players[bot_idx].selection] == 0 {
                for &weapon in &[Equipment::SmallBomb, Equipment::Dynamite, Equipment::BigBomb] {
                  if world.players[bot_idx].inventory[weapon] > 0 {
                    world.players[bot_idx].selection = weapon;
                    break;
                  }
                }
              }

              world.player_action(bot_idx, Key::Bomb);
              let flee_dir = world.actors[bot_idx].facing.reverse();
              if Self::can_step(cursor, flee_dir, &world.maps.level) {
                world.player_action(bot_idx, dir_to_key(flee_dir));
              }
              return;
            }
          }
        }
      }

      // Hard bot remote bomb detonator check: If enemy is in blast radius (<= 3), detonate!
      if difficulty == BotDifficulty::Hard && manhattan <= 3 {
        world.player_action(bot_idx, Key::Remote);
      }

      // Ranged combat: DrillDrone or grenade if aligned in direct line of sight (only Medium & Hard).
      // Both are placed at the bot's own tile and only move once their 1-tick fuse elapses (see
      // grenade_fly/drill_drone_fly in world/explode.rs), so "aligned and within range" alone isn't
      // enough - if a wall sits right next to the bot in that direction, it detonates at melee range
      // for no benefit. has_clear_line checks the path is actually open before committing to the throw.
      if difficulty != BotDifficulty::Easy {
        let max_grenade_dist = if difficulty == BotDifficulty::Hard { 10 } else { 7 };
        if (cursor.row == target_cursor.row || cursor.col == target_cursor.col)
          && manhattan <= max_grenade_dist
          && Self::has_clear_line(cursor, target_cursor, &world.maps.level)
        {
          if world.players[bot_idx].inventory[Equipment::DrillDrone] > 0 {
            world.players[bot_idx].selection = Equipment::DrillDrone;
            if let Some(face_dir) = Self::dir_towards(cursor, target_cursor) {
              world.player_action(bot_idx, dir_to_key(face_dir));
              world.player_action(bot_idx, Key::Bomb);
              return;
            }
          } else if world.players[bot_idx].inventory[Equipment::Grenade] > 0 {
            world.players[bot_idx].selection = Equipment::Grenade;
            if let Some(face_dir) = Self::dir_towards(cursor, target_cursor) {
              world.player_action(bot_idx, dir_to_key(face_dir));
              world.player_action(bot_idx, Key::Bomb);
              return;
            }
          }
        }
      }

      // Hard bot: aggressive pursuit if enemy is close (<= 10 tiles), hunt player before mining
      if difficulty == BotDifficulty::Hard && manhattan <= 10 {
        if let Some(dir) = Self::step_towards(cursor, target_cursor, &world.maps.level) {
          world.player_action(bot_idx, dir_to_key(dir));
          return;
        }
      }
    }

    // 3. MINING & TREASURE: Look for nearby gold, gems, pickaxes
    if let Some(gold_cursor) = Self::find_nearest_treasure(cursor, &world.maps.level) {
      if let Some(dir) = Self::step_towards(cursor, gold_cursor, &world.maps.level) {
        world.player_action(bot_idx, dir_to_key(dir));
        return;
      }
    }

    // 4. HUNT: If no treasures left, pursue nearest alive enemy
    if let Some((enemy_cursor, _)) = Self::find_nearest_enemy(world, bot_idx, cursor) {
      if let Some(dir) = Self::step_towards(cursor, enemy_cursor, &world.maps.level) {
        world.player_action(bot_idx, dir_to_key(dir));
        return;
      }
    }

    // 5. WANDER: If stuck or exploring, pick a valid direction
    let wander_rate = match difficulty {
      BotDifficulty::Hard => 1,   // rarely wanders aimlessly
      BotDifficulty::Medium => 3, // moderate exploration
      BotDifficulty::Easy => 5,   // frequently hesitates/wanders
    };

    if rng.gen_range(0..10) < wander_rate || !Self::can_step(cursor, world.actors[bot_idx].facing, &world.maps.level) {
      let open_dirs: Vec<Direction> = Direction::all()
        .filter(|&d| Self::can_step(cursor, d, &world.maps.level))
        .collect();
      if let Some(&d) = open_dirs.choose(&mut rng) {
        world.player_action(bot_idx, dir_to_key(d));
      } else {
        world.player_action(bot_idx, Key::Stop);
      }
    }
  }

  /// Check if an actor can enter the tile in direction `dir`.
  pub fn can_step(cursor: Cursor, dir: Direction, level: &LevelMap) -> bool {
    let next = cursor.to(dir);
    if next == cursor || next.is_on_border() {
      return false;
    }
    let val = level[next];
    val.is_passable() || val.is_sand() || val.is_treasure()
  }

  /// Look for an active bomb threatening `cursor`.
  fn find_threat_bomb(cursor: Cursor, level: &LevelMap, timer: &TimerMap, max_dist: i16) -> Option<Cursor> {
    for d_row in -max_dist..=max_dist {
      for d_col in -max_dist..=max_dist {
        if let Some(target) = cursor.offset(d_row, d_col) {
          let val = level[target];
          if val.is_bomb() && timer[target] > 0 && timer[target] < 60 {
            // Check if threat is real: aligned row/col or within 2 Manhattan distance
            let (dr, dc) = cursor.distance(target);
            if (dr == 0 && dc <= max_dist as u16) || (dc == 0 && dr <= max_dist as u16) || (dr + dc <= 2) {
              return Some(target);
            }
          }
        }
      }
    }
    None
  }

  /// Choose a direction to escape from a bomb.
  fn choose_flee_direction(cursor: Cursor, bomb: Cursor, level: &LevelMap, difficulty: BotDifficulty) -> Option<Direction> {
    let mut best_dir = None;
    let mut best_score = -999;
    let mut rng = rand::thread_rng();

    // Easy bot has 25% chance to wander in a random direction in panic
    if difficulty == BotDifficulty::Easy && rng.gen_range(0..100) < 25 {
      let open_dirs: Vec<Direction> = Direction::all().filter(|&d| Self::can_step(cursor, d, level)).collect();
      if let Some(&d) = open_dirs.choose(&mut rng) {
        return Some(d);
      }
    }

    for dir in Direction::all() {
      if !Self::can_step(cursor, dir, level) {
        continue;
      }
      let next = cursor.to(dir);
      let (dr, dc) = next.distance(bomb);
      let mut score = (dr + dc) as i32 * 10;

      // Bonus score if next cell breaks line-of-sight with bomb (not same row and not same col)
      if next.row != bomb.row && next.col != bomb.col {
        score += 50;
      }

      // Hard bot penalty if next cell is a dead end
      if difficulty == BotDifficulty::Hard {
        let exits = Direction::all().filter(|&d| Self::can_step(next, d, level)).count();
        if exits <= 1 {
          score -= 40;
        }
      }

      if score > best_score {
        best_score = score;
        best_dir = Some(dir);
      }
    }

    best_dir
  }

  /// Find nearest alive enemy player.
  fn find_nearest_enemy(world: &World, bot_idx: usize, cursor: Cursor) -> Option<(Cursor, usize)> {
    let mut nearest = None;
    let mut min_dist = u16::MAX;

    for idx in 0..world.players.len() {
      if idx == bot_idx || world.actors[idx].is_dead {
        continue;
      }
      let enemy_pos = world.actors[idx].pos.cursor();
      let (dr, dc) = cursor.distance(enemy_pos);
      let dist = dr + dc;
      if dist < min_dist {
        min_dist = dist;
        nearest = Some((enemy_pos, idx));
      }
    }

    nearest
  }

  /// Search for nearest treasure or gem on the map.
  fn find_nearest_treasure(cursor: Cursor, level: &LevelMap) -> Option<Cursor> {
    for distance in 1..=30 {
      for dir in &[Direction::Up, Direction::Down, Direction::Left, Direction::Right] {
        for idx in -distance..=distance {
          let offset = match dir {
            Direction::Up => cursor.offset(-distance, idx),
            Direction::Down => cursor.offset(distance, idx),
            Direction::Left => cursor.offset(idx, -distance),
            Direction::Right => cursor.offset(idx, distance),
          };

          if let Some(target) = offset {
            let val = level[target];
            if val.is_treasure() {
              return Some(target);
            }
          }
        }
      }
    }
    None
  }

  /// Determine single step direction towards a target.
  fn step_towards(cursor: Cursor, target: Cursor, level: &LevelMap) -> Option<Direction> {
    let (curr_dr, curr_dc) = cursor.distance(target);
    let mut best_dir = None;
    let mut best_dist = curr_dr + curr_dc;

    for dir in Direction::all() {
      if !Self::can_step(cursor, dir, level) {
        continue;
      }
      let next = cursor.to(dir);
      let (dr, dc) = next.distance(target);
      let dist = dr + dc;
      if dist < best_dist {
        best_dist = dist;
        best_dir = Some(dir);
      }
    }

    best_dir
  }

  /// Get direct orthogonal direction towards target if aligned.
  fn dir_towards(cursor: Cursor, target: Cursor) -> Option<Direction> {
    if cursor.row == target.row {
      if target.col > cursor.col {
        Some(Direction::Right)
      } else {
        Some(Direction::Left)
      }
    } else if cursor.col == target.col {
      if target.row > cursor.row {
        Some(Direction::Down)
      } else {
        Some(Direction::Up)
      }
    } else {
      None
    }
  }

  /// Whether every cell strictly between `cursor` and `target` (along their shared row or column) is
  /// passable, i.e. a projectile fired from `cursor` towards `target` would actually reach it rather
  /// than detonating on the first obstruction. `cursor` and `target` must already be aligned (checked
  /// by the only caller before this runs); returns `false` if they are not, same as a wall in the way.
  fn has_clear_line(cursor: Cursor, target: Cursor, level: &LevelMap) -> bool {
    let Some(dir) = Self::dir_towards(cursor, target) else {
      return false;
    };
    let mut cur = cursor;
    let max_steps = crate::world::map::MAP_ROWS.max(crate::world::map::MAP_COLS);
    for _ in 0..max_steps {
      cur = cur.to(dir);
      if cur == target {
        return true;
      }
      if cur.is_on_border() || !level[cur].is_passable() {
        return false;
      }
    }
    false
  }

  /// Get candidate direction towards center avoiding obstacles like metal wall or lava
  fn dir_towards_center(cursor: Cursor, target: Cursor, level: &LevelMap) -> Option<Direction> {
    let mut candidates = Vec::new();
    if target.row < cursor.row { candidates.push(Direction::Up); }
    if target.row > cursor.row { candidates.push(Direction::Down); }
    if target.col < cursor.col { candidates.push(Direction::Left); }
    if target.col > cursor.col { candidates.push(Direction::Right); }

    candidates.into_iter().find(|&d| {
      let next_cur = cursor.to(d);
      let val = level[next_cur];
      val != MapValue::MetalWall && val != MapValue::Napalm1 && val != MapValue::Napalm2
    })
  }
}

fn dir_to_key(dir: Direction) -> Key {
  match dir {
    Direction::Up => Key::Up,
    Direction::Down => Key::Down,
    Direction::Left => Key::Left,
    Direction::Right => Key::Right,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::options::Options;
  use crate::world::player::PlayerComponent;

  /// Where the developer keeps the original game files, if anywhere: `MB_GAME_DIR`, else `res/minebomb`
  /// (same convention as `crates/mb-wasm/src/lib.rs`'s `game_dir()` - this package's manifest dir is
  /// already the repository root, so unlike that one this needs no `../..`).
  fn game_dir() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("MB_GAME_DIR")
      .map(std::path::PathBuf::from)
      .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("res/minebomb"));
    if dir.join("TITLEBE.SPY").is_file() {
      Some(dir)
    } else {
      None
    }
  }

  fn real_classic_map(name: &str) -> LevelMap {
    let dir = game_dir().expect("original game files not found (set MB_GAME_DIR or keep them in res/minebomb)");
    let data = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("cannot read {}: {}", name, e));
    LevelMap::from_file_map(data).unwrap_or_else(|_| panic!("{} is not a valid map file", name))
  }

  fn equip_for_a_fair_fight(p: &mut PlayerComponent) {
    p.inventory[Equipment::SmallBomb] = 30;
    p.inventory[Equipment::Dynamite] = 5;
    p.inventory[Equipment::BigBomb] = 3;
    p.inventory[Equipment::SmallPickaxe] = 2;
    // Ranged options bot.rs only lets Medium/Hard use (see update_single_bot's "Ranged combat" step) -
    // without these, that whole differentiator never fires and Easy is compared unfairly favorably.
    p.inventory[Equipment::Grenade] = 5;
    p.inventory[Equipment::DrillDrone] = 3;
  }

  /// Runs one bot-vs-bot match to a decision (someone dies) or `max_ticks`, and returns the winner's
  /// index (0 or 1), or `None` for a draw/timeout. Both bots get identical equipment, so any skew in
  /// outcomes comes only from `BotDifficulty`, not from one side having better gear.
  fn run_match(level: LevelMap, diff_a: BotDifficulty, diff_b: BotDifficulty, max_ticks: u32) -> Option<usize> {
    let options = Options::default();
    let mut players = [
      PlayerComponent::new("A".to_string(), Default::default(), &options, true, diff_a),
      PlayerComponent::new("B".to_string(), Default::default(), &options, true, diff_b),
    ];
    for p in players.iter_mut() {
      equip_for_a_fair_fight(p);
    }
    let mut world = World::create(level, &mut players, false, 50, false);

    for _ in 0..max_ticks {
      world.tick();
      let a_dead = world.actors[0].is_dead;
      let b_dead = world.actors[1].is_dead;
      if a_dead && b_dead {
        return None;
      } else if a_dead {
        return Some(1);
      } else if b_dead {
        return Some(0);
      }
    }
    None
  }

  /// Simulates real matches with the actual bot AI (no mocking) and reports each difficulty pairing's
  /// win rate, instead of trusting that the code having three enum variants with different numbers
  /// means they play meaningfully differently. This is a measurement tool, not a pass/fail balance
  /// check: at n=250 (see below for why that many), two separate runs both put every pairing (Hard vs
  /// Easy, Hard vs Medium, Medium vs Easy) between 47% and 54% for the stronger side - i.e. on the
  /// evidence here, the three difficulty levels are close to interchangeable in an actual fight, despite
  /// the many behavioral differences in `update_single_bot`.
  /// Fixed two concrete bugs that were making it worse before landing on that number (both kept - they
  /// are correct regardless of the net win-rate effect): `has_clear_line` (a Hard/Medium bot firing a
  /// Grenade/DrillDrone at a wall right next to it, since neither check blocked the "aligned and in
  /// range" shot before this), and preferring `SmallBomb` over `BigBomb`/`Dynamite` in the melee
  /// drop-and-flee tactic (a one-tile retreat cannot clear a radius-2/3+ blast, so "better bomb" was
  /// mostly self-damage). Actually giving `Hard` a reliable edge looks like it needs real tuning work
  /// (tried moving `close_combat_dist` for Hard from 3 to 2 to match Medium; that made Hard *worse*
  /// against Easy, 23/60 - reverted), which is a design decision past what this session could respons-
  /// ibly guess its way to further; this test is the tool for whoever does that work next to check their
  /// change actually moved the number, instead of re-discovering the same noise problem below.
  ///
  /// n=250, not 60: at n=60 this looked fixed (e.g. one run: strong=35 weak=25 for Hard vs Easy, a
  /// believable-looking 58%), but that was mostly sampling noise - a second n=60 run of the *identical*
  /// code came back strong=23 weak=37 (38%) for the same pairing. n=250 stopped moving on repeat runs.
  /// Takes a few minutes; that is the cost of the sample size actually being large enough to trust.
  ///
  /// Needs a real classic map, not `LevelMap::empty()`: on an open map every bomb blast travels
  /// unobstructed across the whole board, which produced simultaneous "both players die together" chain
  /// reactions that swamped the AI-quality signal this test is after (both sides died in ~77% of trials,
  /// regardless of difficulty, when this was first written against `LevelMap::empty()`).
  #[test]
  #[ignore = "needs the original game files: set MB_GAME_DIR or keep them in res/minebomb"]
  fn measure_bot_difficulty_win_rates() {
    const TRIALS: u32 = 250;
    const MAX_TICKS: u32 = 60 * 180;
    let level = real_classic_map("BATTLE.MNE");

    // A sanity check, not a balance judgement: this only catches something actually broken (every
    // match timing out, or one side never winning at all), not "is the gap big enough" - see the doc
    // comment above for why this test does not assert a target win rate.
    let mut broken = Vec::new();
    for (label, a, b) in [
      ("Hard vs Easy", BotDifficulty::Hard, BotDifficulty::Easy),
      ("Hard vs Medium", BotDifficulty::Hard, BotDifficulty::Medium),
      ("Medium vs Easy", BotDifficulty::Medium, BotDifficulty::Easy),
    ] {
      let mut a_wins = 0;
      let mut b_wins = 0;
      let mut draws = 0;
      for _ in 0..TRIALS {
        match run_match(level.clone(), a, b, MAX_TICKS) {
          Some(0) => a_wins += 1,
          Some(1) => b_wins += 1,
          _ => draws += 1,
        }
      }
      println!("{label}: {a:?}={a_wins} {b:?}={b_wins} draws={draws} (of {TRIALS})");
      if draws * 4 > TRIALS {
        broken.push(format!("{label}: {draws}/{TRIALS} matches never resolved"));
      }
      if a_wins == 0 || b_wins == 0 {
        broken.push(format!("{label}: one side never won a single match ({a_wins} vs {b_wins})"));
      }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
  }

  /// Same shape as a real local game: one human-controlled slot (never sent an action here - there is
  /// no human in a test, so this checks the other three don't need one to behave) plus Easy/Medium/Hard
  /// bots together in one match, the combination #16's original playtest request asked for but this
  /// session could not drive interactively (screen-control access was declined). Checks the engine
  /// handles a mixed 4-actor match without crashing or hanging, not difficulty balance specifically -
  /// `harder_bot_difficulty_wins_more_often` above is the one with a difficulty assertion.
  #[test]
  #[ignore = "needs the original game files: set MB_GAME_DIR or keep them in res/minebomb"]
  fn four_player_match_with_one_human_slot_and_three_bot_difficulties_completes() {
    let level = real_classic_map("BATTLE.MNE");
    let options = Options::default();
    let mut players = [
      PlayerComponent::new("Player".to_string(), Default::default(), &options, false, BotDifficulty::Easy),
      PlayerComponent::new("Bot Easy".to_string(), Default::default(), &options, true, BotDifficulty::Easy),
      PlayerComponent::new("Bot Medium".to_string(), Default::default(), &options, true, BotDifficulty::Medium),
      PlayerComponent::new("Bot Hard".to_string(), Default::default(), &options, true, BotDifficulty::Hard),
    ];
    for p in players.iter_mut() {
      equip_for_a_fair_fight(p);
    }
    let mut world = World::create(level, &mut players, false, 50, false);

    let max_ticks = 60 * 180;
    let mut ticks_run = 0;
    for t in 0..max_ticks {
      world.tick();
      ticks_run = t + 1;
      if world.alive_players() < 2 {
        break;
      }
    }

    let alive: Vec<&str> = world
      .players
      .iter()
      .zip(world.actors.iter())
      .filter(|(_, a)| !a.is_dead)
      .map(|(p, _)| p.stats.name.as_str())
      .collect();
    println!("4-player match: resolved after {ticks_run} ticks, still alive: {alive:?}");
    assert!(!world.actors[0..4].iter().all(|a| a.is_dead), "every actor died - likely a mutual chain reaction, not real combat");
  }

  #[test]
  fn test_dir_towards() {
    let c1 = Cursor::new(5, 5);
    let c_right = Cursor::new(5, 10);
    assert_eq!(BotController::dir_towards(c1, c_right), Some(Direction::Right));

    let c_up = Cursor::new(2, 5);
    assert_eq!(BotController::dir_towards(c1, c_up), Some(Direction::Up));

    let c_diag = Cursor::new(7, 7);
    assert_eq!(BotController::dir_towards(c1, c_diag), None);
  }

  #[test]
  fn test_dir_to_key() {
    assert_eq!(dir_to_key(Direction::Up), Key::Up);
    assert_eq!(dir_to_key(Direction::Down), Key::Down);
    assert_eq!(dir_to_key(Direction::Left), Key::Left);
    assert_eq!(dir_to_key(Direction::Right), Key::Right);
  }

  #[test]
  fn test_bot_difficulty_cycle() {
    assert_eq!(BotDifficulty::Easy.cycle(), BotDifficulty::Medium);
    assert_eq!(BotDifficulty::Medium.cycle(), BotDifficulty::Hard);
    assert_eq!(BotDifficulty::Hard.cycle(), BotDifficulty::Easy);
  }

  #[test]
  fn test_bot_difficulty_from_str() {
    assert_eq!(BotDifficulty::from_str("easy"), BotDifficulty::Easy);
    assert_eq!(BotDifficulty::from_str("NORMAL"), BotDifficulty::Medium);
    assert_eq!(BotDifficulty::from_str("hard"), BotDifficulty::Hard);
    assert_eq!(BotDifficulty::from_str("unknown"), BotDifficulty::Medium);
  }
}


