use crate::world::actor::{ActorComponent, ActorKind, Player};
use crate::world::map::{LevelMap, MapValue};
use crate::world::position::{Cursor, Direction};
use crate::world::{grenade_value, EntityIndex, World};
use rand::prelude::*;

impl World<'_> {
  /// Animate non-player actors
  pub(super) fn animate_monsters(&mut self) {
    let remaining_gold = self.gold_remaining();
    for actor_idx in self.players.len()..self.actors.len() {
      let monster = &self.actors[actor_idx];
      let monster_kind = monster.kind;
      let monster_cursor = monster.pos.cursor();
      if !monster.is_active || monster.is_dead {
        // Monster is not active or dead
        continue;
      }

      self.damage_players(actor_idx);

      if self.classic_tick() % monster_kind.speed() != 0 {
        self.animate_actor(actor_idx);
      }

      // The original keeps a movement command and a facing direction and copies the command into
      // the facing here whenever it is not "stand still". Only the command moves the monster; the
      // facing only picks the sprite of a monster that stands still (MB.EXE segment 1 at 0x621c).
      // `facing` with `moving` carries both: `moving == false` is the "stand still" command.

      if self.classic_tick() % 26 == 0 {
        if let Some(bomb_cursor) = look_for_bombs(monster_cursor, &self.maps.level) {
          self.actors[actor_idx].avoid_position(bomb_cursor, &self.maps.level);
        } else {
          let seen = look_for_players(monster_cursor, &self.actors[0..self.players.len()]);
          match (monster_kind, seen) {
            // Clones shouldn't chase their player!
            (ActorKind::Clone(_), Some((player_cursor, player_idx)))
              if self.clone_can_chase(monster_kind, player_idx) =>
            {
              self.actors[actor_idx].head_to_target(player_cursor, &self.maps.level);
              // Clones throw grenades only when actually locked on somebody
              self.grenadier_maybe_toss_grenade(actor_idx);
            }
            (ActorKind::Clone(owner), seen) => {
              // Clones look for gold! With no gold left, the original sends a clone that sees its
              // own player after that player (segment 1 at 0x9471).
              let target = if remaining_gold > 0 {
                look_for_gold(monster_cursor, &self.maps.level)
              } else {
                seen
                  .filter(|&(_, player_idx)| player_idx == owner)
                  .map(|(player_cursor, _)| player_cursor)
              };
              if let Some(target) = target {
                self.actors[actor_idx].head_to_target(target, &self.maps.level);
              }
            }
            (_, Some((player_cursor, _))) => {
              self.actors[actor_idx].head_to_target(player_cursor, &self.maps.level);
            }
            (_, None) => {}
          }

          // Grenadiers always throw grenades (unless avoiding bombs)
          if monster_kind == ActorKind::Grenadier {
            self.grenadier_maybe_toss_grenade(actor_idx);
          }
        }
      }

      let actor = &self.actors[actor_idx];
      if (self.classic_tick() % 33 == 0 && actor.blocked(&self.maps.level)) || self.classic_tick() % 121 == 0 {
        let mut rng = rand::thread_rng();
        let dir = *[Direction::Left, Direction::Right, Direction::Up, Direction::Down]
          .choose(&mut rng)
          .unwrap();
        self.actors[actor_idx].moving = true;
        self.actors[actor_idx].facing = dir;
      }
    }
  }

  fn clone_can_chase(&self, monster_kind: ActorKind, target_player: Player) -> bool {
    match monster_kind {
      ActorKind::Clone(clone_player) if clone_player != target_player && !self.campaign_mode && !self.survival_mode => {
        true
      }
      ActorKind::Clone(_) => false,
      _ => true,
    }
  }

  /// Make given actor to cause damage to all players in the same cell
  fn damage_players(&mut self, actor: EntityIndex) {
    let cursor = self.actors[actor].pos.cursor();
    let monster_kind = self.actors[actor].kind;
    for player_idx in 0..self.players.len() {
      let player = &mut self.actors[player_idx];
      if player.pos.cursor() == cursor {
        match (player.kind, monster_kind) {
          (ActorKind::Player(p1), ActorKind::Clone(p2)) if p1 == p2 || self.campaign_mode || self.survival_mode => {
            // Nothing! This is our clone! Also, no damage in campaign or survival mode.
          }
          _ => {
            player.health = player.health.saturating_sub(monster_kind.damage());
            self.update.update_player_health(player_idx);
          }
        }
      }
    }
  }

  /// Throw grenades if not blocked by map or by other monster
  ///
  /// Mirrors MB.EXE segment 1 at 0x8cba: with more than 4 clear cells ahead, the monster throws
  /// when any player is in its row or its column (not both). A monster that stands still has no
  /// command to throw along and does not throw.
  fn grenadier_maybe_toss_grenade(&mut self, actor: EntityIndex) {
    // Minimum distance to obstacle when grenadier still wants to throw a grenade
    const MIN_OBSTACLE_DISTANCE: i32 = 4;

    let actor = &self.actors[actor];
    if actor.moving && self.check_obstacle_distance(actor) > MIN_OBSTACLE_DISTANCE {
      let cursor = actor.pos.cursor();
      for player in &self.actors[..self.players.len()] {
        let player_cursor = player.pos.cursor();
        let same_row = player_cursor.row == cursor.row;
        let same_col = player_cursor.col == cursor.col;
        if same_row != same_col {
          self.maps.level[cursor] = grenade_value(actor.facing);
          self.maps.timer[cursor] = 1;
        }
      }
    }
  }

  /// How many cells ahead of the monster a grenade could fly through.
  ///
  /// Output: 0..=10.
  ///
  /// Mirrors MB.EXE segment 1 at 0x8af0. Walking up to 10 cells ahead, each passable cell counts
  /// one. A cell that is not passable ends the walk without counting; a cell with a monster in it
  /// ends the walk after counting. A clone whose own player is ahead gets 0, so it never throws
  /// over them.
  fn check_obstacle_distance(&self, actor: &ActorComponent) -> i32 {
    const MAX_SCAN_DISTANCE: i32 = 10;

    let mut cursor = actor.pos.cursor();
    let mut count = 0;
    for _ in 0..MAX_SCAN_DISTANCE {
      cursor = cursor.to(actor.facing);
      let passable = self.maps.level[cursor].is_passable();
      if passable {
        count += 1;
      }

      match actor.kind {
        ActorKind::Clone(player) if self.actors[player as usize].pos.cursor() == cursor => {
          return 0;
        }
        _ => {}
      }

      // Some monster is blocking grenade throw
      let monster_here = self.actors[self.players.len()..]
        .iter()
        .any(|actor| actor.pos.cursor() == cursor);
      if !passable || monster_here {
        return count;
      }
    }
    count
  }
}

/// Look around for bombs (MB.EXE segment 1 at 0x7dbe)
fn look_for_bombs(cursor: Cursor, level: &LevelMap) -> Option<Cursor> {
  look_around(cursor, 5, |offset, _| {
    let value = level[offset];
    if value.is_bomb() {
      return Some(offset);
    }
    None
  })
}

/// Look around for players (MB.EXE segment 1 at 0x81fe)
///
/// When players share a cell, the original checks players 1 to 3 in order and then player 4, who
/// wins over the others.
fn look_for_players(cursor: Cursor, players: &[ActorComponent]) -> Option<(Cursor, Player)> {
  look_around(cursor, 10, |offset, _| {
    let mut found = None;
    for player in players {
      if player.pos.cursor() == offset {
        if let ActorKind::Player(player_idx) = player.kind {
          if found.is_none() || player_idx == Player::Player4 {
            found = Some((player.pos.cursor(), player_idx));
          }
        } else {
          unreachable!();
        }
      }
    }
    found
  })
}

/// Look around for gold (MB.EXE segment 1 at 0x8e28)
///
/// The original skips pickaxes and drills in the row above the monster, and only there.
fn look_for_gold(cursor: Cursor, level: &LevelMap) -> Option<Cursor> {
  look_around(cursor, 63, |offset, side| {
    let value = level[offset];
    let tool = value == MapValue::SmallPickaxe || value == MapValue::LargePickaxe || value == MapValue::Drill;
    if value.is_treasure() || (tool && side != Direction::Up) {
      return Some(offset);
    }
    None
  })
}

/// Look around in growing squares, checking each side (above, below, left, right) in turn.
///
/// Input: the centre, the largest distance, and a check that gets each cell and the side it is on.
/// Output: what the check found on the first side that has a match.
///
/// Mirrors the searches in MB.EXE segment 1: they walk a whole side and keep the last match on it
/// (top to bottom, left to right) before moving to the next side, so the last match wins, not the
/// first.
fn look_around<T>(cursor: Cursor, distance: i16, check_location: impl Fn(Cursor, Direction) -> Option<T>) -> Option<T> {
  for distance in 1..=distance {
    for dir in [Direction::Up, Direction::Down, Direction::Left, Direction::Right] {
      let mut found = None;
      for idx in -distance..=distance {
        let offset = match dir {
          Direction::Up => cursor.offset(-distance, idx),
          Direction::Down => cursor.offset(distance, idx),
          Direction::Left => cursor.offset(idx, -distance),
          Direction::Right => cursor.offset(idx, distance),
        };

        if let Some(target) = offset.and_then(|offset| check_location(offset, dir)) {
          found = Some(target);
        }
      }
      if found.is_some() {
        return found;
      }
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::options::Options;
  use crate::world::bot::BotDifficulty;
  use crate::world::player::PlayerComponent;

  fn humans(count: usize) -> Vec<PlayerComponent> {
    let options = Options::default();
    (0..count)
      .map(|idx| {
        PlayerComponent::new(
          format!("P{}", idx),
          Default::default(),
          &options,
          false,
          BotDifficulty::Medium,
        )
      })
      .collect()
  }

  fn monster(kind: ActorKind, cursor: Cursor, facing: Direction) -> ActorComponent {
    ActorComponent {
      kind,
      pos: cursor.into(),
      health: 100,
      facing,
      moving: true,
      is_active: true,
      ..Default::default()
    }
  }

  #[test]
  fn search_keeps_the_last_match_on_a_side() {
    // Two pieces of gold two rows up: the original scans the row left to right and keeps the last.
    let mut level = LevelMap::empty();
    level[Cursor::new(8, 9)] = MapValue::GoldBar;
    level[Cursor::new(8, 11)] = MapValue::GoldBar;
    assert_eq!(look_for_gold(Cursor::new(10, 10), &level), Some(Cursor::new(8, 11)));
  }

  #[test]
  fn gold_search_skips_tools_only_above() {
    let from = Cursor::new(10, 10);
    let mut level = LevelMap::empty();
    level[Cursor::new(9, 10)] = MapValue::Drill;
    assert_eq!(look_for_gold(from, &level), None);

    let mut level = LevelMap::empty();
    level[Cursor::new(11, 10)] = MapValue::Drill;
    assert_eq!(look_for_gold(from, &level), Some(Cursor::new(11, 10)));
  }

  #[test]
  fn player_four_wins_a_shared_cell() {
    let cell = Cursor::new(10, 12);
    let players: Vec<ActorComponent> = [Player::Player1, Player::Player2, Player::Player3, Player::Player4]
      .iter()
      .map(|&player| ActorComponent {
        kind: ActorKind::Player(player),
        pos: cell.into(),
        ..Default::default()
      })
      .collect();
    let seen = look_for_players(Cursor::new(10, 10), &players).map(|(_, player)| player);
    assert!(seen == Some(Player::Player4));
    let seen = look_for_players(Cursor::new(10, 10), &players[..3]).map(|(_, player)| player);
    assert!(seen == Some(Player::Player1));
  }

  #[test]
  fn a_monster_standing_still_is_never_blocked_and_stays_put_on_a_level_bomb() {
    let level = {
      let mut level = LevelMap::empty();
      level[Cursor::new(10, 11)] = MapValue::MetalWall;
      level
    };
    let mut furry = monster(ActorKind::Furry, Cursor::new(10, 10), Direction::Right);
    assert!(furry.blocked(&level));
    furry.moving = false;
    assert!(!furry.blocked(&level));

    // A bomb in its own cell gives neither axis a direction, so the original keeps "stand still".
    for _ in 0..200 {
      furry.avoid_position(Cursor::new(10, 10), &level);
      assert!(!furry.moving);
    }
  }

  #[test]
  fn grenade_path_counts_the_cell_a_monster_stands_in() {
    let mut players = humans(2);
    let mut world = World::create(LevelMap::empty(), &mut players, false, 50, false);
    world.actors[0].pos = Cursor::new(40, 60).into();
    world.actors[1].pos = Cursor::new(40, 61).into();
    world
      .actors
      .push(monster(ActorKind::Grenadier, Cursor::new(10, 10), Direction::Right));
    world
      .actors
      .push(monster(ActorKind::Furry, Cursor::new(10, 13), Direction::Left));
    let grenadier = world.actors[2].clone();
    assert_eq!(world.check_obstacle_distance(&grenadier), 3);
  }

  #[test]
  fn a_clone_never_counts_a_clear_path_over_its_own_player() {
    let mut players = humans(2);
    let mut world = World::create(LevelMap::empty(), &mut players, false, 50, false);
    world.actors[0].pos = Cursor::new(10, 17).into();
    world.actors[1].pos = Cursor::new(40, 61).into();
    let clone = monster(ActorKind::Clone(Player::Player1), Cursor::new(10, 10), Direction::Right);
    assert_eq!(world.check_obstacle_distance(&clone), 0);
  }

  #[test]
  fn a_clone_with_no_gold_left_follows_its_own_player() {
    let mut players = humans(2);
    let mut world = World::create(LevelMap::empty(), &mut players, false, 50, false);
    world.actors[0].pos = Cursor::new(10, 15).into();
    world.actors[1].pos = Cursor::new(40, 61).into();
    world.actors.push(monster(
      ActorKind::Clone(Player::Player1),
      Cursor::new(10, 10),
      Direction::Left,
    ));
    assert_eq!(world.gold_remaining(), 0);

    // 26 is a decision tick and neither a `% 33` nor a `% 121` one.
    world.round_counter = 26;
    world.animate_monsters();
    assert!(world.actors[2].facing == Direction::Right && world.actors[2].moving);
  }

  #[test]
  fn decisions_run_on_the_one_byte_tick() {
    let mut players = humans(2);
    let mut world = World::create(LevelMap::empty(), &mut players, false, 50, false);
    world.actors[0].pos = Cursor::new(10, 15).into();
    world.actors[1].pos = Cursor::new(40, 61).into();
    world
      .actors
      .push(monster(ActorKind::Furry, Cursor::new(10, 10), Direction::Left));

    // 256 + 26 = 282 is not a multiple of 26, but the original's byte counter reads 26 there.
    world.round_counter = 256 + 26;
    assert_eq!(world.classic_tick(), 26);
    world.animate_monsters();
    assert!(world.actors[2].facing == Direction::Right);
  }

  #[test]
  fn players_three_and_four_are_noticed_on_their_own_tick() {
    let mut players = humans(3);
    let mut world = World::create(LevelMap::empty(), &mut players, false, 50, false);
    world.actors[0].pos = Cursor::new(40, 60).into();
    world.actors[1].pos = Cursor::new(40, 61).into();
    world.actors[2].pos = Cursor::new(10, 10).into();
    let mut furry = monster(ActorKind::Furry, Cursor::new(10, 10), Direction::Left);
    furry.is_active = false;
    world.actors.push(furry);

    // `% 5 == 0` looks for players 1 and 2 only.
    world.round_counter = 5;
    world.tick();
    assert!(!world.actors[3].is_active);

    // `% 5 == 3` looks for players 3 and 4.
    world.round_counter = 8;
    world.tick();
    assert!(world.actors[3].is_active);
  }
}
