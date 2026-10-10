use crate::effects::SoundEffect;
use crate::world::map::{LevelMap, MapValue};
use crate::world::position::{Cursor, Direction, Position};
use rand::prelude::*;
use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Player {
  Player1 = 0,
  Player2 = 1,
  Player3 = 2,
  Player4 = 3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ActorKind {
  Furry,
  Grenadier,
  Slime,
  Alien,
  Player(Player),
  Clone(Player),
}

impl ActorKind {
  pub fn drilling_power(self) -> u16 {
    match self {
      ActorKind::Furry => 5,
      ActorKind::Grenadier => 12,
      ActorKind::Slime => 12,
      ActorKind::Alien => 52,
      ActorKind::Clone(_) => 52,
      _ => unimplemented!(),
    }
  }

  pub fn initial_health(self) -> u16 {
    match self {
      ActorKind::Furry => 29,
      ActorKind::Grenadier => 29,
      ActorKind::Slime => 10,
      ActorKind::Alien => 66,
      ActorKind::Clone(_) => 100,
      _ => unimplemented!(),
    }
  }

  pub fn damage(self) -> u16 {
    match self {
      ActorKind::Furry => 2,
      ActorKind::Grenadier => 3,
      ActorKind::Slime => 1,
      ActorKind::Alien => 5,
      ActorKind::Clone(_) => 1,
      // Players don't do damage by hands!
      ActorKind::Player(_) => 0,
    }
  }

  pub fn speed(self) -> usize {
    match self {
      ActorKind::Furry => 6,
      ActorKind::Grenadier => 3,
      ActorKind::Slime => 2,
      ActorKind::Alien => 100,
      ActorKind::Clone(_) => 100,
      _ => unimplemented!(),
    }
  }

  pub fn blood_value(self) -> MapValue {
    match self {
      ActorKind::Slime => MapValue::SlimeCorpse,
      _ => MapValue::Blood,
    }
  }

  pub fn death_animation_value(self) -> MapValue {
    match self {
      ActorKind::Slime => MapValue::SlimeDying,
      _ => MapValue::MonsterDying,
    }
  }

  pub fn death_sound_effect(self) -> SoundEffect {
    match self {
      ActorKind::Slime => SoundEffect::Urethan,
      _ => SoundEffect::Aargh,
    }
  }
}

/// Actor component is an active entity on the map. It has position, visual representation,
/// digging power and health.
#[derive(Clone)]
pub struct ActorComponent {
  pub kind: ActorKind,
  pub facing: Direction,
  pub moving: bool,
  /// Maximum health
  pub max_health: u16,
  /// Current health
  pub health: u16,
  pub pos: Position,
  pub drilling: u16,
  pub animation: u8,
  pub is_dead: bool,
  /// If monster is active
  pub is_active: bool,
  /// Cash accumulated in the current map; will be lost on death.
  pub accumulated_cash: u32,
  /// Countdown of player activated acceleration bonus
  pub super_drill_count: u32,
  /// Countdown of frozen state (cannot move while > 0)
  pub frozen_ticks: u16,
}

impl Default for ActorComponent {
  fn default() -> Self {
    ActorComponent {
      kind: ActorKind::Furry,
      facing: Direction::Right,
      moving: false,
      max_health: 0,
      health: 0,
      pos: Position { x: 0, y: 0 },
      drilling: 0,
      animation: 0,
      is_dead: false,
      is_active: false,
      accumulated_cash: 0,
      super_drill_count: 0,
      frozen_ticks: 0,
    }
  }
}

impl ActorComponent {
  /// Check if we can continue moving in the current direction
  pub fn can_move(&self, level: &LevelMap) -> bool {
    let next = self.pos.cursor().to(self.facing);
    let value = level[next];
    value.is_passable() || value.is_sand() || value.is_treasure()
  }

  /// Whether a monster is walking into a cell it cannot enter.
  ///
  /// Output: false for a monster that stands still, whatever is in front of it.
  ///
  /// The original keeps a monster's movement as a command where 0 means "stand still", and its
  /// blocked check (MB.EXE segment 1 at 0x83a2) answers "not blocked" for 0. `moving == false` is
  /// that 0 here, so a monster that stopped stays stopped until something gives it a direction.
  pub fn blocked(&self, level: &LevelMap) -> bool {
    self.moving && !self.can_move(level)
  }

  /// Step in `dir`, or keep the current command when there is no direction to pick (`None`).
  fn command(&mut self, dir: Option<Direction>) {
    if let Some(dir) = dir {
      self.facing = dir;
      self.moving = true;
    }
  }

  /// Actively avoid given location
  ///
  /// Mirrors MB.EXE segment 1 at 0x854e: run along the axis the bomb is farther away on (or, 3 times
  /// in 100, sideways anyway), and turn when blocked. A monster level with the bomb on that axis
  /// keeps its command, which may be "stand still".
  pub fn avoid_position(&mut self, bomb: Cursor, level: &LevelMap) {
    let cursor = self.pos.cursor();
    let mut rng = rand::thread_rng();
    let (delta_row, delta_col) = cursor.distance(bomb);

    if delta_col > delta_row || rng.gen_range(0..100) < 3 {
      self.command(match cursor.col.cmp(&bomb.col) {
        Ordering::Greater => Some(Direction::Right),
        Ordering::Less => Some(Direction::Left),
        Ordering::Equal => None,
      });

      if self.blocked(level) {
        self.command(Some(Direction::Down));
      }
      if self.blocked(level) {
        self.command(Some(Direction::Up));
      }
    } else {
      self.command(match cursor.row.cmp(&bomb.row) {
        Ordering::Greater => Some(Direction::Down),
        Ordering::Less => Some(Direction::Up),
        Ordering::Equal => None,
      });

      if self.blocked(level) {
        self.command(Some(Direction::Left));
      }
      if self.blocked(level) {
        self.command(Some(Direction::Right));
      }
    }
  }

  /// Head for the given location
  ///
  /// Mirrors MB.EXE segment 1 at 0x86c7: go along the longer axis first, the shorter one when
  /// blocked, and a random command (stand still included) when still blocked.
  pub fn head_to_target(&mut self, target: Cursor, level: &LevelMap) {
    let cursor = self.pos.cursor();
    let (delta_row, delta_col) = cursor.distance(target);
    let horizontal = |cursor: Cursor| match cursor.col.cmp(&target.col) {
      Ordering::Greater => Some(Direction::Left),
      Ordering::Less => Some(Direction::Right),
      Ordering::Equal => None,
    };
    let vertical = |cursor: Cursor| match cursor.row.cmp(&target.row) {
      Ordering::Greater => Some(Direction::Up),
      Ordering::Less => Some(Direction::Down),
      Ordering::Equal => None,
    };

    // Try going for longer direction first
    if delta_col > delta_row {
      self.command(horizontal(cursor));
    } else {
      self.command(vertical(cursor));
    }

    // If blocked, try going for shorter direction
    if self.blocked(level) {
      if delta_col <= delta_row {
        self.command(horizontal(cursor));
      } else {
        self.command(vertical(cursor));
      }
    }

    // If blocked, choose random direction!
    if self.blocked(level) {
      let mut rng = rand::thread_rng();

      // Note that this is a bit different than other place we go in random direction
      // Here it is possible to choose "stop" randomly
      let dir = *[
        None,
        Some(Direction::Left),
        Some(Direction::Right),
        Some(Direction::Up),
        Some(Direction::Down),
      ]
      .choose(&mut rng)
      .unwrap();
      if let Some(dir) = dir {
        self.facing = dir;
      } else {
        self.moving = false;
      }
    }
  }
}
