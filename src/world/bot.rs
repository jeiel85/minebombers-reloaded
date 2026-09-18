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
              // Select weapon based on difficulty preference
              if difficulty == BotDifficulty::Hard {
                if world.players[bot_idx].inventory[Equipment::BigBomb] > 0 {
                  world.players[bot_idx].selection = Equipment::BigBomb;
                } else if world.players[bot_idx].inventory[Equipment::Dynamite] > 0 {
                  world.players[bot_idx].selection = Equipment::Dynamite;
                } else if world.players[bot_idx].inventory[Equipment::Mine] > 0 {
                  world.players[bot_idx].selection = Equipment::Mine;
                } else if world.players[bot_idx].inventory[Equipment::SmallBomb] > 0 {
                  world.players[bot_idx].selection = Equipment::SmallBomb;
                }
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

      // Ranged combat: DrillDrone or grenade if aligned in direct line of sight (only Medium & Hard)
      if difficulty != BotDifficulty::Easy {
        let max_grenade_dist = if difficulty == BotDifficulty::Hard { 10 } else { 7 };
        if (cursor.row == target_cursor.row || cursor.col == target_cursor.col) && manhattan <= max_grenade_dist {
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


