//! Bot AI for Mine Bombers
//!
//! The bot plays the same game a human does: it *digs*. Mine Bombers maps are mostly solid rock with
//! the gold buried inside it, so an AI that only walks through pre-existing passages has almost
//! nothing to do - which is exactly what the first version of this file did (`can_step` accepted only
//! passable/sand/treasure tiles, so no branch of the decision tree could ever target a buried gem).
//! Measured on 20 random maps x 3000 ticks, that bot visited ~9 distinct tiles, dug ~4 and picked up
//! under two treasures per round, on every difficulty. The rewrite below takes that to 40-59 tiles
//! visited, 20-50 dug and 4-9 treasures, and the three difficulties now differ (Hard digs 50 tiles a
//! round to Easy's 20): see `measure_bot_activity` and `measure_bot_difficulty_win_rates`.
//!
//! The decision loop, in priority order:
//!
//! 1. **Survive** - a shared [`DangerMap`] marks every tile some live explosive will cover and how
//!    many ticks remain before it does, so "am I in a blast?" is an exact lookup rather than the old
//!    "is a bomb roughly aligned with me?" heuristic. If the tile is covered, walk to the nearest
//!    tile that is not (and that we can reach before the fuse runs out).
//! 2. **Fight** - drop a bomb only when an enemy is actually inside its blast pattern *and* a retreat
//!    tile exists, throw grenades/drill drones down clear lines, detonate remote bombs, plant mines.
//! 3. **Mine** - Dijkstra over the whole map with real costs in ticks (10 per tile walked, `hits /
//!    drilling` per tile dug), pick the target with the best value-per-tick, and walk/dig towards it.
//!    Bombs are used as digging tools when hand-digging the next tile would take too long.
//! 4. **Never freeze** - progress (tile changed, or `hits` of the tile being dug went down) is tracked
//!    every decision; when it stalls the plan is thrown away and the bot digs the cheapest neighbour.
//!    Every fallback ends in a movement action; standing still is never one of the choices. Issue #20
//!    (bot alive but motionless for 1800+ ticks, at row 1) turned out not to be a freeze at all but an
//!    oscillation: a bot that re-planned in the middle of a step could reverse, land back where it
//!    started and repeat, so its tile never changed. Hence the COMMIT step - once a step is under way,
//!    only danger or a shot worth taking may interrupt it.
//!
//! Difficulty is *not* a separate AI per level - all three run the code above, and [`BotParams`]
//! scales how well they run it (how often they think, how far they plan, how much of the danger map
//! they respect, whether they check for an escape route before bombing, and how often they simply
//! fumble). That is deliberate: two earlier attempts to separate the difficulties by nudging
//! individual constants on top of a weak shared core failed to move win rates at all (see
//! `measure_bot_difficulty_win_rates`), because the core was the limiting factor, not the constants.

use crate::keys::Key;
use crate::world::equipment::Equipment;
use crate::world::explode::{BIG_BOMB_PATTERN, DYNAMITE_PATTERN, SMALL_BOMB_PATTERN};
use crate::world::map::{MapValue, MAP_COLS, MAP_ROWS};
use crate::world::position::{Cursor, Direction};
use crate::world::World;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

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

/// Number of cells on the map, for the flat arrays used by the danger map and the path search.
const MAP_CELLS: usize = (MAP_ROWS as usize) * (MAP_COLS as usize);

/// [`DangerMap`] entry for a cell no live explosive covers.
const SAFE: u16 = u16::MAX;

/// Ticks an actor needs to cross one tile: `Position::step` advances 1 unit per tick and a tile is
/// 10 units wide (see `world::animate_actor`). Every cost in this file is in ticks, so that walking
/// and digging are directly comparable.
const WALK_TICKS: u32 = 10;

/// Ticks without progress (neither moving to another tile nor reducing the `hits` of the tile being
/// dug) after which the bot discards its plan and forces itself out of the spot.
const STUCK_LIMIT: u32 = 90;

/// Ticks a bot waits after dropping a bomb before it considers dropping another one. Longer than a
/// small bomb's 100-tick fuse (see `item_placement_timer`) so it never buries itself in its own bombs.
const BOMB_COOLDOWN: u32 = 110;

/// Ticks between ranged shots. Short - a grenade is gone from our tile immediately - but not zero.
const RANGED_COOLDOWN: u32 = 25;

/// Closest an enemy may be for a grenade or drill drone to be worth firing: these detonate on the
/// target and the blast reaches one tile past it, so anything nearer catches the bot too.
const RANGED_MIN_DISTANCE: u16 = 3;

/// How well a bot plays the shared decision loop. See the module docs for why difficulty is expressed
/// this way instead of as separate behavior per level.
#[derive(Debug, Clone, Copy)]
struct BotParams {
  /// Ticks between decisions. 1 = reacts on every tick.
  decision_interval: usize,
  /// Ticks a plan is kept before the target is re-evaluated.
  replan_interval: u32,
  /// Maximum path cost (in ticks) the target search will consider - how far ahead the bot can plan.
  search_budget: u32,
  /// Maximum cost (in ticks) of hand-digging a single tile before the bot treats it as a wall.
  /// With bare hands (drilling 1) this is what separates "can only dig sand" from "can tunnel
  /// through solid stone"; a drill or pickaxe raises `drilling` and lifts all three at once.
  dig_cost_cap: u32,
  /// A blast arriving within this many ticks counts as a threat worth reacting to.
  danger_horizon: u16,
  /// Extra ticks of slack required between arriving at a tile and a blast reaching it.
  escape_margin: u32,
  /// How many tiles of walking the escape/retreat search looks at.
  escape_steps: u32,
  /// Whether the bot confirms it has somewhere to run before dropping a bomb.
  verify_retreat: bool,
  /// Whether the bot throws grenades / launches drill drones.
  use_ranged: bool,
  /// Whether the bot detonates its own remote bombs.
  use_remote: bool,
  /// Whether the bot plants mines in an approaching enemy's path.
  use_mines: bool,
  /// Whether the bot blasts through rock that would take too long to hand-dig.
  bomb_digging: bool,
  /// Hand-digging cost (ticks) above which bombing the tile is preferred.
  bomb_dig_threshold: u32,
  /// Percent chance per decision of moving at random instead of following the plan.
  mistake_pct: u32,
  /// Percent chance per decision of ignoring a blast that covers the current tile.
  ignore_danger_pct: u32,
  /// Value of an enemy as a target, against the treasure values in `cell_value` (a gold bar is 60, a
  /// diamond 330). This is what makes Hard hunt while Easy mostly digs - and it is the one knob that
  /// has to be kept in check rather than maximized for the strongest bot: at 220 (an enemy worth more
  /// than any treasure on the map) Hard spent whole rounds at point blank range and killed *itself*
  /// in 42 of 150 rounds against an opponent that never fought back, against 9 of 150 at 120.
  enemy_value: u32,
  /// Maximum distance to fire a grenade / drill drone.
  ranged_range: u16,
  /// Whether the bot weighs an enemy by how the fight would go: press the attack while ahead on
  /// health, go back to mining while behind. Fighting an armed opponent is a coin flip at the best
  /// of times, so picking *when* to take it is the one thing left that separates a strong bot from
  /// an attentive one - `measure_bot_difficulty_win_rates` had Hard and Medium dead even without it.
  situational_aggression: bool,
}

impl BotParams {
  fn for_difficulty(difficulty: BotDifficulty) -> Self {
    match difficulty {
      // Thinks slowly, plans barely past its own cavern, only digs what bare hands dig quickly
      // (sand and light gravel), notices bombs only once the fuse is nearly out, never checks
      // whether it has somewhere to run, and fumbles one decision in five.
      BotDifficulty::Easy => BotParams {
        decision_interval: 5,
        replan_interval: 70,
        search_budget: 320,
        dig_cost_cap: 180,
        danger_horizon: 28,
        escape_margin: 0,
        escape_steps: 4,
        verify_retreat: false,
        use_ranged: false,
        use_remote: false,
        use_mines: false,
        bomb_digging: false,
        bomb_dig_threshold: u32::MAX,
        mistake_pct: 22,
        ignore_danger_pct: 30,
        enemy_value: 50,
        situational_aggression: false,
        ranged_range: 0,
      },
      BotDifficulty::Medium => BotParams {
        decision_interval: 2,
        replan_interval: 30,
        search_budget: 900,
        dig_cost_cap: 900,
        danger_horizon: 60,
        escape_margin: 10,
        escape_steps: 6,
        verify_retreat: true,
        use_ranged: true,
        use_remote: false,
        use_mines: false,
        bomb_digging: true,
        bomb_dig_threshold: 150,
        mistake_pct: 7,
        ignore_danger_pct: 5,
        enemy_value: 100,
        situational_aggression: false,
        ranged_range: 7,
      },
      // Reacts every tick, plans across the whole map, tunnels through solid stone, respects the
      // full fuse length of every blast, never fumbles, and picks its fights by who is winning them.
      BotDifficulty::Hard => BotParams {
        decision_interval: 1,
        replan_interval: 14,
        search_budget: 4000,
        dig_cost_cap: 4000,
        danger_horizon: 110,
        escape_margin: 20,
        escape_steps: 8,
        verify_retreat: true,
        use_ranged: true,
        use_remote: true,
        use_mines: true,
        bomb_digging: true,
        bomb_dig_threshold: 110,
        mistake_pct: 0,
        ignore_danger_pct: 0,
        enemy_value: 150,
        situational_aggression: true,
        ranged_range: 10,
      },
    }
  }
}

/// Per-bot memory that has to survive between decisions: the current plan, and enough history to
/// notice that the bot is making no progress.
#[derive(Debug, Default, Clone)]
pub struct BotState {
  /// Remaining waypoints towards the goal, nearest first. Each is adjacent to the previous one.
  path: Vec<Cursor>,
  /// Tile the plan is aimed at, kept so the plan can be dropped when the target disappears.
  goal: Option<Cursor>,
  /// Whether that tile was picked for something lying on it (treasure, tool, crate) rather than for
  /// an enemy standing there. An item can be taken by someone else, and then the plan is pointless;
  /// an enemy simply moves, which the normal replan interval handles.
  goal_is_item: bool,
  /// Ticks since the plan was made.
  plan_age: u32,
  last_cursor: Option<Cursor>,
  /// Tile the bot was facing last decision, and its `hits` then - digging it lowers `hits` without
  /// moving the bot, which is progress even though the position is unchanged.
  last_front: Option<Cursor>,
  last_front_hits: i32,
  stuck_ticks: u32,
  bomb_cooldown: u32,
}

impl BotState {
  /// Forget the current plan, so the next decision picks a fresh target.
  fn clear_plan(&mut self) {
    self.path.clear();
    self.goal = None;
    self.goal_is_item = false;
  }
}

/// For every cell, how many ticks until some live explosive covers it ([`SAFE`] if none will).
///
/// Blast patterns in this game are fixed offsets applied regardless of what is in the way (see
/// `explode_pattern`), so a bomb's footprint is exactly known in advance - walls do not stop it, and
/// neither should the bot pretend they do.
pub struct DangerMap {
  ticks: Vec<u16>,
  /// Tiles an active monster stands on or can reach out of: monsters damage by contact, every tick
  /// they touch you (2-5 points each, see `ActorKind::damage`), which is slow enough that no single
  /// tick looks dangerous and deadly enough to kill a bot that lingers. Tracked separately from
  /// `ticks` because it is not a blast with a fuse - it is a place not to stand.
  contact: Vec<bool>,
}

impl DangerMap {
  /// Build the danger map for the current tick. Shared by every bot, so this runs once per tick.
  pub fn compute(world: &World) -> Self {
    let mut map = DangerMap {
      ticks: vec![SAFE; MAP_CELLS],
      contact: vec![false; MAP_CELLS],
    };

    for (idx, actor) in world.actors.iter().enumerate() {
      if idx < world.players.len() || actor.is_dead || !actor.is_active {
        continue;
      }
      let cursor = actor.pos.cursor();
      map.contact[cell_index(cursor)] = true;
      for dir in Direction::all() {
        let next = cursor.to(dir);
        if next != cursor {
          map.contact[cell_index(next)] = true;
        }
      }
    }

    for cursor in Cursor::all() {
      let value = world.maps.level[cursor];
      let timer = world.maps.timer[cursor];
      if value.is_bomb() && timer > 0 {
        map.mark_blast(cursor, value, timer);
      } else if matches!(value, MapValue::Explosion | MapValue::Napalm1 | MapValue::Napalm2) {
        map.mark(cursor, 0);
      }
    }

    // Sudden death: the collapsed rings are lava, and the next ring in is about to become lava.
    if world.sudden_death_active && world.sudden_death_ring > 0 {
      let ring = world.sudden_death_ring;
      for cursor in Cursor::all() {
        if world.is_in_danger_zone(cursor) {
          map.mark(cursor, 0);
        } else if cursor.row <= ring + 2
          || cursor.row >= MAP_ROWS - 1 - (ring + 2)
          || cursor.col <= ring + 2
          || cursor.col >= MAP_COLS - 1 - (ring + 2)
        {
          map.mark(cursor, 90);
        }
      }
    }

    map
  }

  /// Ticks until a blast covers `cursor`, or [`SAFE`].
  pub fn at(&self, cursor: Cursor) -> u16 {
    self.ticks[cell_index(cursor)]
  }

  /// Whether an active monster is on `cursor` or next to it.
  pub fn monster_contact(&self, cursor: Cursor) -> bool {
    self.contact[cell_index(cursor)]
  }

  fn mark(&mut self, cursor: Cursor, ticks: u16) {
    let index = cell_index(cursor);
    if self.ticks[index] > ticks {
      self.ticks[index] = ticks;
    }
  }

  fn mark_offsets(&mut self, center: Cursor, offsets: &[(i16, i16)], ticks: u16) {
    self.mark(center, ticks);
    for (delta_row, delta_col) in offsets {
      if let Some(cursor) = center.offset(*delta_row, *delta_col) {
        self.mark(cursor, ticks);
      }
    }
  }

  fn mark_disc(&mut self, center: Cursor, radius: i16, ticks: u16) {
    for delta_row in -radius..=radius {
      for delta_col in -radius..=radius {
        if delta_row * delta_row + delta_col * delta_col > radius * radius {
          continue;
        }
        if let Some(cursor) = center.offset(delta_row, delta_col) {
          self.mark(cursor, ticks);
        }
      }
    }
  }

  fn mark_line(&mut self, center: Cursor, dir: Direction, length: u16, ticks: u16) {
    let mut cursor = center;
    for _ in 0..length {
      let next = cursor.to(dir);
      if next == cursor || next.is_on_border() {
        break;
      }
      cursor = next;
      self.mark(cursor, ticks);
    }
  }

  /// Footprint of one explosive. Mirrors the patterns `explode_entity` dispatches on; the expanding
  /// ones (napalm, plastic, digger) and the atomic bomb's circle are approximated with a disc, which
  /// is the point - the bot should give them a wide berth, not model them exactly.
  fn mark_blast(&mut self, center: Cursor, value: MapValue, ticks: u16) {
    match value {
      MapValue::SmallBomb1
      | MapValue::SmallBomb2
      | MapValue::SmallBomb3
      | MapValue::SmallBombExtinguished
      | MapValue::MetalWallPlaced => self.mark_offsets(center, &SMALL_BOMB_PATTERN, ticks),

      MapValue::BigBomb1
      | MapValue::BigBomb2
      | MapValue::BigBomb3
      | MapValue::BigBombExtinguished
      | MapValue::SmallRadioBlue
      | MapValue::SmallRadioRed
      | MapValue::SmallRadioGreen
      | MapValue::SmallRadioYellow
      | MapValue::ExplosivePlastic
      | MapValue::Barrel => self.mark_offsets(center, &BIG_BOMB_PATTERN, ticks),

      MapValue::Dynamite1
      | MapValue::Dynamite2
      | MapValue::Dynamite3
      | MapValue::DynamiteExtinguished
      | MapValue::BigRadioBlue
      | MapValue::BigRadioRed
      | MapValue::BigRadioGreen
      | MapValue::BigRadioYellow => self.mark_offsets(center, &DYNAMITE_PATTERN, ticks),

      MapValue::Atomic1 | MapValue::Atomic2 | MapValue::Atomic3 => self.mark_disc(center, 12, ticks),

      MapValue::Napalm1
      | MapValue::Napalm2
      | MapValue::NapalmExtinguished
      | MapValue::PlasticBomb
      | MapValue::ExplosivePlasticBomb
      | MapValue::DiggerBomb => self.mark_disc(center, 4, ticks),

      MapValue::BlackHoleBomb | MapValue::BlackHoleActive => self.mark_disc(center, 3, ticks),
      MapValue::FreezeBomb | MapValue::JumpingBomb => self.mark_disc(center, 2, ticks),

      MapValue::SmallCrucifixBomb => {
        self.mark(center, ticks);
        for dir in Direction::all() {
          self.mark_line(center, dir, 15, ticks);
        }
      }
      MapValue::LargeCrucifixBomb => {
        self.mark(center, ticks);
        for dir in Direction::all() {
          self.mark_line(center, dir, MAP_COLS, ticks);
        }
      }

      // Grenades and drill drones travel in their facing direction before going off.
      MapValue::GrenadeFlyingRight | MapValue::DrillDroneRight => {
        self.mark_offsets(center, &SMALL_BOMB_PATTERN, ticks);
        self.mark_line(center, Direction::Right, 8, ticks);
      }
      MapValue::GrenadeFlyingLeft | MapValue::DrillDroneLeft => {
        self.mark_offsets(center, &SMALL_BOMB_PATTERN, ticks);
        self.mark_line(center, Direction::Left, 8, ticks);
      }
      MapValue::GrenadeFlyingUp | MapValue::DrillDroneUp => {
        self.mark_offsets(center, &SMALL_BOMB_PATTERN, ticks);
        self.mark_line(center, Direction::Up, 8, ticks);
      }
      MapValue::GrenadeFlyingDown | MapValue::DrillDroneDown => {
        self.mark_offsets(center, &SMALL_BOMB_PATTERN, ticks);
        self.mark_line(center, Direction::Down, 8, ticks);
      }

      _ => self.mark_offsets(center, &SMALL_BOMB_PATTERN, ticks),
    }
  }
}

/// Cost-to-reach and predecessor for every cell, from one Dijkstra run rooted at the bot.
struct PathField {
  dist: Vec<u32>,
  parent: Vec<u32>,
}

impl PathField {
  /// Waypoints from (but not including) `start` up to and including `goal`, or an empty path if
  /// `goal` was never reached.
  fn path_to(&self, start: Cursor, goal: Cursor) -> Vec<Cursor> {
    if self.dist[cell_index(goal)] == u32::MAX {
      return Vec::new();
    }
    let mut path = Vec::new();
    let mut cursor = goal;
    while cursor != start {
      path.push(cursor);
      let parent = self.parent[cell_index(cursor)];
      if parent == u32::MAX {
        return Vec::new();
      }
      cursor = cell_at(parent as usize);
    }
    path.reverse();
    path
  }
}

/// What a bot decided to do this tick: optionally switch weapon, then press some keys.
#[derive(Default)]
struct BotAction {
  select: Option<Equipment>,
  keys: Vec<Key>,
}

impl BotAction {
  fn step(dir: Direction) -> Self {
    BotAction {
      select: None,
      keys: vec![dir_to_key(dir)],
    }
  }

  fn key(key: Key) -> Self {
    BotAction {
      select: None,
      keys: vec![key],
    }
  }
}

pub struct BotController;

impl BotController {
  /// Update AI decisions for all active bot players.
  pub fn update_bots(world: &mut World) {
    if world.bots.len() < world.players.len() {
      world.bots.resize_with(world.players.len(), BotState::default);
    }
    if !world.players.iter().any(|player| player.is_bot) {
      return;
    }

    let danger = DangerMap::compute(world);

    for bot_idx in 0..world.players.len() {
      if !world.players[bot_idx].is_bot || world.actors[bot_idx].is_dead || world.actors[bot_idx].frozen_ticks > 0 {
        continue;
      }
      let params = BotParams::for_difficulty(world.players[bot_idx].bot_difficulty);
      if (world.round_counter + bot_idx) % params.decision_interval != 0 {
        continue;
      }

      // The state is taken out so `decide` can borrow the world immutably; it is put back before
      // any action is applied.
      let mut state = std::mem::take(&mut world.bots[bot_idx]);
      let action = Self::decide(world, bot_idx, &danger, &params, &mut state);
      world.bots[bot_idx] = state;

      if let Some(weapon) = action.select {
        if world.players[bot_idx].inventory[weapon] > 0 {
          world.players[bot_idx].selection = weapon;
        }
      }
      for key in action.keys {
        world.player_action(bot_idx, key);
      }
    }
  }

  fn decide(world: &World, bot_idx: usize, danger: &DangerMap, params: &BotParams, state: &mut BotState) -> BotAction {
    let cursor = world.actors[bot_idx].pos.cursor();
    let facing = world.actors[bot_idx].facing;
    let drilling = u32::from(world.actors[bot_idx].drilling.max(1));
    let elapsed = params.decision_interval as u32;
    let mut rng = rand::thread_rng();

    state.bomb_cooldown = state.bomb_cooldown.saturating_sub(elapsed);
    state.plan_age += elapsed;

    // Progress tracking: either we changed tile, or we are chipping away at the tile in front of us.
    let front = cursor.to(facing);
    let front_hits = world.maps.hits[front];
    let progressed =
      state.last_cursor != Some(cursor) || (state.last_front == Some(front) && front_hits < state.last_front_hits);
    if progressed {
      state.stuck_ticks = 0;
    } else {
      state.stuck_ticks += elapsed;
    }
    state.last_cursor = Some(cursor);
    state.last_front = Some(front);
    state.last_front_hits = front_hits;

    // Waypoints we already stand on are done.
    while state.path.first() == Some(&cursor) {
      state.path.remove(0);
    }

    // 1. SURVIVE: step out of anything that is about to explode.
    let here = danger.at(cursor);
    let panics = params.ignore_danger_pct > 0 && rng.gen_range(0..100) < params.ignore_danger_pct;
    if here <= params.danger_horizon && !panics {
      if let Some(dir) = Self::escape_direction(world, cursor, danger, params) {
        state.clear_plan();
        return BotAction::step(dir);
      }
    }

    // 2. FIGHT.
    if let Some(action) = Self::combat_action(world, bot_idx, cursor, danger, params, state) {
      return action;
    }

    // 3. BACK OFF: a monster next to us takes a bite every tick. Trading that for a bomb is fine;
    // standing there because the bomb is on cooldown, or because the plan says to dig here, is not.
    if danger.monster_contact(cursor) && state.bomb_cooldown > 0 {
      if let Some(dir) = Self::walk_search(world, cursor, params.escape_steps, |cell, _| {
        !danger.monster_contact(cell) && danger.at(cell) == SAFE
      }) {
        state.clear_plan();
        return BotAction::step(dir);
      }
    }

    // 4. COMMIT: an actor between two tiles that reverses direction ends up exactly where it
    // started, and a bot that re-plans every few ticks can do that for hundreds of ticks in a row -
    // the oscillation behind issue #20's "alive but never moves". Once a step is under way, finish
    // it; the danger and combat steps above are the only things allowed to interrupt a step.
    if is_between_tiles(&world.actors[bot_idx]) {
      let ahead = cursor.to(facing);
      if ahead != cursor && enter_cost(world, ahead, drilling, danger, params, 0).is_some() {
        return BotAction::step(facing);
      }
    }

    // 5. FUMBLE: weaker bots misplay a fraction of their decisions outright. Only at tile centers,
    // so that a fumble is a wrong turn rather than a reversal that undoes the step in progress.
    if params.mistake_pct > 0 && rng.gen_range(0..100) < params.mistake_pct {
      if let Some(dir) = Self::random_open_direction(world, cursor, danger, drilling, params, &mut rng) {
        return BotAction::step(dir);
      }
    }

    // 6. UNSTICK: no progress for a while - the plan is not working, force a way out.
    if state.stuck_ticks >= STUCK_LIMIT {
      state.clear_plan();
      state.stuck_ticks = 0;
      return Self::unstick(world, bot_idx, cursor, danger, drilling, params, &mut rng);
    }

    // 7. PLAN: pick the best value-per-tick target reachable within the search budget.
    let stale = state.plan_age >= params.replan_interval
      || state.path.is_empty()
      || (state.goal_is_item
        && state
          .goal
          .is_none_or(|goal| Self::cell_value(world, bot_idx, goal) == 0));
    if stale {
      Self::replan(world, bot_idx, cursor, danger, drilling, params, state);
    }

    // 8. FOLLOW: walk, dig, or blast the next waypoint open.
    if let Some(&next) = state.path.first() {
      if let Some(dir) = direction_to_adjacent(cursor, next) {
        if params.bomb_digging
          && state.bomb_cooldown == 0
          && dig_ticks(world, next, drilling) > params.bomb_dig_threshold
        {
          if let Some(action) = Self::bomb_dig(world, bot_idx, cursor, danger, params, state) {
            return action;
          }
        }
        return BotAction::step(dir);
      }
      // Plan no longer starts next to us (teleport, push, blast) - drop it.
      state.clear_plan();
    }

    // 9. Nothing reachable is worth anything: keep digging rather than stand still.
    Self::unstick(world, bot_idx, cursor, danger, drilling, params, &mut rng)
  }

  // ---------------------------------------------------------------------------------------------
  // Survival
  // ---------------------------------------------------------------------------------------------

  /// First step of the shortest walk to a tile no blast will reach before we get there.
  fn escape_direction(world: &World, cursor: Cursor, danger: &DangerMap, params: &BotParams) -> Option<Direction> {
    let margin = params.escape_margin;
    let escape = Self::walk_search(world, cursor, params.escape_steps, |cell, arrival| {
      let when = danger.at(cell);
      when == SAFE || u32::from(when) > arrival + margin
    });
    if escape.is_some() {
      return escape;
    }

    // Boxed in: at least move to whichever neighbour the blast reaches last.
    let mut best: Option<(u16, Direction)> = None;
    for dir in Direction::all() {
      let next = cursor.to(dir);
      if next == cursor || !is_walkable(world, next) {
        continue;
      }
      let when = danger.at(next);
      if best.is_none_or(|(best_when, _)| when > best_when) {
        best = Some((when, dir));
      }
    }
    best.filter(|&(when, _)| when > danger.at(cursor)).map(|(_, dir)| dir)
  }

  /// Breadth-first walk (no digging - digging is far too slow to run away with) over at most
  /// `max_steps` tiles, returning the first step towards the nearest accepted tile.
  fn walk_search<F>(world: &World, start: Cursor, max_steps: u32, accept: F) -> Option<Direction>
  where
    F: Fn(Cursor, u32) -> bool,
  {
    let mut visited = vec![false; MAP_CELLS];
    let mut queue = VecDeque::new();
    visited[cell_index(start)] = true;
    queue.push_back((start, 0u32, None::<Direction>));

    while let Some((cell, steps, first_dir)) = queue.pop_front() {
      if steps > 0 && accept(cell, steps * WALK_TICKS) {
        return first_dir;
      }
      if steps >= max_steps {
        continue;
      }
      for dir in Direction::all() {
        let next = cell.to(dir);
        if next == cell || visited[cell_index(next)] || !is_walkable(world, next) {
          continue;
        }
        visited[cell_index(next)] = true;
        queue.push_back((next, steps + 1, first_dir.or(Some(dir))));
      }
    }
    None
  }

  /// Somewhere to run to after dropping `weapon` at our feet: a reachable tile outside its blast that
  /// is not already threatened by something else. The clearance needed depends on the bomb - a small
  /// bomb reaches one tile, dynamite three - so this is what stops a bot from "escaping" a dynamite
  /// stick by stepping one tile sideways.
  fn retreat_direction(
    world: &World,
    cursor: Cursor,
    danger: &DangerMap,
    params: &BotParams,
    weapon: Equipment,
  ) -> Option<Direction> {
    let clearance = blast_clearance(weapon);
    Self::walk_search(world, cursor, params.escape_steps, |cell, _| {
      let (delta_row, delta_col) = cursor.distance(cell);
      delta_row + delta_col > clearance && danger.at(cell) == SAFE
    })
  }

  // ---------------------------------------------------------------------------------------------
  // Combat
  // ---------------------------------------------------------------------------------------------

  fn combat_action(
    world: &World,
    bot_idx: usize,
    cursor: Cursor,
    danger: &DangerMap,
    params: &BotParams,
    state: &mut BotState,
  ) -> Option<BotAction> {
    let (target, _target_idx) = Self::nearest_enemy(world, bot_idx, cursor)?;
    let (delta_row, delta_col) = cursor.distance(target);
    let manhattan = delta_row + delta_col;

    // Remote bombs: fire only when one of ours is next to the enemy and not next to us.
    if params.use_remote && Self::remote_is_worth_firing(world, bot_idx, cursor, target) {
      return Some(BotAction::key(Key::Remote));
    }

    let on_bombable_tile = world.maps.level[cursor].is_passable();

    // Point blank: the enemy stands inside the blast of a bomb dropped at our feet.
    if manhattan <= 1 && state.bomb_cooldown == 0 && on_bombable_tile {
      if let Some(weapon) = Self::melee_weapon(world, bot_idx) {
        let retreat = Self::retreat_direction(world, cursor, danger, params, weapon);
        if retreat.is_some() || !params.verify_retreat {
          state.bomb_cooldown = BOMB_COOLDOWN;
          state.clear_plan();
          let mut keys = vec![Key::Bomb];
          if let Some(dir) = retreat {
            keys.push(dir_to_key(dir));
          }
          return Some(BotAction {
            select: Some(weapon),
            keys,
          });
        }
      }
    }

    // Mines are armed by contact rather than a fuse, so they are the one explosive that can be left
    // in a chaser's path without any risk to the bot that planted it.
    if params.use_mines
      && (2..=4).contains(&manhattan)
      && state.bomb_cooldown == 0
      && on_bombable_tile
      && world.players[bot_idx].inventory[Equipment::Mine] > 0
    {
      state.bomb_cooldown = BOMB_COOLDOWN;
      return Some(BotAction {
        select: Some(Equipment::Mine),
        keys: vec![Key::Bomb],
      });
    }

    // Ranged: grenades and drill drones are placed on our own tile and only start travelling once
    // their 1-tick fuse elapses, so an obstruction right next to us means they detonate in our face.
    // They also go off *at the target*, and their blast reaches a tile further still - firing at
    // something two tiles away is shooting ourselves, which is why this has a minimum range as well
    // as a maximum one.
    if params.use_ranged
      && (cursor.row == target.row || cursor.col == target.col)
      && (RANGED_MIN_DISTANCE..=params.ranged_range).contains(&manhattan)
      && state.bomb_cooldown == 0
      && on_bombable_tile
      && Self::has_clear_line(cursor, target, world)
    {
      let weapon = if world.players[bot_idx].inventory[Equipment::DrillDrone] > 0 {
        Some(Equipment::DrillDrone)
      } else if world.players[bot_idx].inventory[Equipment::Grenade] > 0 {
        Some(Equipment::Grenade)
      } else {
        None
      };
      if let Some(weapon) = weapon {
        if let Some(face) = Self::dir_towards(cursor, target) {
          // Without this the bot fires one per decision for as long as the line holds, which empties
          // a shop's worth of grenades in a couple of seconds.
          state.bomb_cooldown = RANGED_COOLDOWN;
          return Some(BotAction {
            select: Some(weapon),
            keys: vec![dir_to_key(face), Key::Bomb],
          });
        }
      }
    }

    None
  }

  /// Bomb to drop at point blank range. A small bomb covers only the four neighbouring tiles, which
  /// a one-tile retreat escapes; the bigger ones are last resorts, since their radius-2/3 blast is
  /// as likely to catch the bot as its target.
  fn melee_weapon(world: &World, bot_idx: usize) -> Option<Equipment> {
    let inventory = &world.players[bot_idx].inventory;
    [Equipment::SmallBomb, Equipment::BigBomb, Equipment::Dynamite]
      .iter()
      .copied()
      .find(|&weapon| inventory[weapon] > 0)
  }

  /// Whether one of this player's remote bombs is placed well enough to be worth detonating now.
  fn remote_is_worth_firing(world: &World, bot_idx: usize, cursor: Cursor, target: Cursor) -> bool {
    Cursor::all().any(|cell| {
      if !crate::world::is_remote_for(world.maps.level[cell], bot_idx) {
        return false;
      }
      let (target_row, target_col) = cell.distance(target);
      let (own_row, own_col) = cell.distance(cursor);
      target_row + target_col <= 2 && own_row + own_col >= 3
    })
  }

  fn nearest_enemy(world: &World, bot_idx: usize, cursor: Cursor) -> Option<(Cursor, usize)> {
    let mut nearest = None;
    let mut min_dist = u16::MAX;

    for idx in 0..world.actors.len() {
      if idx == bot_idx || world.actors[idx].is_dead {
        continue;
      }
      // Monsters only count once they have noticed someone; idle ones are scenery.
      if idx >= world.players.len() && !world.actors[idx].is_active {
        continue;
      }
      let enemy = world.actors[idx].pos.cursor();
      let (delta_row, delta_col) = cursor.distance(enemy);
      let dist = delta_row + delta_col;
      if dist < min_dist {
        min_dist = dist;
        nearest = Some((enemy, idx));
      }
    }

    nearest
  }

  // ---------------------------------------------------------------------------------------------
  // Planning
  // ---------------------------------------------------------------------------------------------

  /// Dijkstra from the bot over the whole map, with costs in ticks: walking is [`WALK_TICKS`] per
  /// tile, digging adds `hits / drilling`, and threatened tiles are expensive (or, when the blast is
  /// imminent, impassable).
  fn path_field(world: &World, start: Cursor, drilling: u32, danger: &DangerMap, params: &BotParams) -> PathField {
    let mut field = PathField {
      dist: vec![u32::MAX; MAP_CELLS],
      parent: vec![u32::MAX; MAP_CELLS],
    };
    let mut heap = BinaryHeap::new();
    field.dist[cell_index(start)] = 0;
    heap.push(Reverse((0u32, cell_index(start) as u32)));

    while let Some(Reverse((dist, index))) = heap.pop() {
      if dist > field.dist[index as usize] {
        continue;
      }
      let cell = cell_at(index as usize);
      for dir in Direction::all() {
        let next = cell.to(dir);
        if next == cell {
          continue;
        }
        let Some(step) = enter_cost(world, next, drilling, danger, params, dist) else {
          continue;
        };
        let next_dist = dist + step;
        if next_dist <= params.search_budget && next_dist < field.dist[cell_index(next)] {
          field.dist[cell_index(next)] = next_dist;
          field.parent[cell_index(next)] = index;
          heap.push(Reverse((next_dist, cell_index(next) as u32)));
        }
      }
    }

    field
  }

  fn replan(
    world: &World,
    bot_idx: usize,
    cursor: Cursor,
    danger: &DangerMap,
    drilling: u32,
    params: &BotParams,
    state: &mut BotState,
  ) {
    let field = Self::path_field(world, cursor, drilling, danger, params);

    // (utility, tile, is_item)
    let mut best: Option<(u64, Cursor, bool)> = None;
    let consider = |value: u32, cell: Cursor, is_item: bool, best: &mut Option<(u64, Cursor, bool)>| {
      let cost = field.dist[cell_index(cell)];
      if cost == u32::MAX || value == 0 {
        return;
      }
      // Value per tick invested, with a floor on the denominator so that a treasure two tiles away
      // does not outrank everything else purely by being close.
      let utility = u64::from(value) * 1000 / (u64::from(cost) + 60);
      if best.is_none_or(|(best_utility, ..)| utility > best_utility) {
        *best = Some((utility, cell, is_item));
      }
    };

    for cell in Cursor::all_without_borders() {
      consider(Self::cell_value(world, bot_idx, cell), cell, true, &mut best);
    }

    for idx in 0..world.actors.len() {
      if idx == bot_idx || world.actors[idx].is_dead {
        continue;
      }
      let is_monster = idx >= world.players.len();
      if is_monster && (!world.actors[idx].is_active || !world.survival_mode) {
        // Hunting monsters for their own sake is how a bot bleeds to death: they hit back on contact
        // and are worth nothing outside Survival Horde, where clearing the wave *is* the objective.
        continue;
      }
      let value = if params.situational_aggression {
        Self::weigh_enemy(world, bot_idx, idx, params.enemy_value)
      } else {
        params.enemy_value
      };
      consider(value, world.actors[idx].pos.cursor(), false, &mut best);
    }

    // Nothing of value in range: keep heading wherever we were already heading if that is still
    // reachable, and only otherwise pick a new direction to explore in. Re-deciding this every
    // `replan_interval` is what makes a bot with nothing to do walk back and forth on the spot.
    let goal = best
      .map(|(_, cell, _)| cell)
      .or_else(|| {
        state
          .goal
          .filter(|goal| *goal != cursor && field.dist[cell_index(*goal)] != u32::MAX)
      })
      .or_else(|| {
        let mut furthest: Option<(u32, Cursor)> = None;
        for cell in Cursor::all_without_borders() {
          let cost = field.dist[cell_index(cell)];
          if cost == u32::MAX || cost == 0 {
            continue;
          }
          if furthest.is_none_or(|(best_cost, _)| cost > best_cost) {
            furthest = Some((cost, cell));
          }
        }
        furthest.map(|(_, cell)| cell)
      });

    state.plan_age = 0;
    state.goal = goal;
    state.goal_is_item = best.is_some_and(|(_, cell, is_item)| is_item && Some(cell) == goal);
    state.path = goal.map(|goal| field.path_to(cursor, goal)).unwrap_or_default();
  }

  /// How much this enemy is worth going after right now, given how the two sides stand. Health is
  /// the only part of an opponent's strength that is visible on the map (their inventory is not), and
  /// it is the part that decides a bomb exchange.
  fn weigh_enemy(world: &World, bot_idx: usize, enemy_idx: usize, base: u32) -> u32 {
    let own = u32::from(world.actors[bot_idx].health);
    let theirs = u32::from(world.actors[enemy_idx].health).max(1);
    if own * 4 < theirs * 3 {
      // Losing an exchange: break off and go back to mining.
      base / 4
    } else if own > theirs * 3 / 2 {
      // Clearly ahead: finish it.
      base * 3 / 2
    } else {
      base
    }
  }

  /// Worth of a tile as a destination, on the same scale as [`BotParams::enemy_value`].
  fn cell_value(world: &World, bot_idx: usize, cell: Cursor) -> u32 {
    let value = world.maps.level[cell];
    if value.is_treasure() {
      return 30 + value.gold_value().min(300);
    }
    match value {
      // Digging tools pay for themselves many times over: `drilling` divides every future dig cost.
      MapValue::SmallPickaxe => 70,
      MapValue::LargePickaxe => 130,
      MapValue::Drill => 200,
      MapValue::WeaponsCrate => 80,
      MapValue::Medikit => {
        let actor = &world.actors[bot_idx];
        if u32::from(actor.health) * 2 < u32::from(actor.max_health) {
          300
        } else {
          40
        }
      }
      MapValue::LifeItem => 100,
      MapValue::Exit if world.campaign_mode => 150,
      _ => 0,
    }
  }

  /// Blast the tile ahead instead of hand-digging it. The bot drops the bomb at its own feet (the
  /// only place it can), which cracks the surrounding rock - stone drops from 2000 hits to 500/1000
  /// (see `explode_cell`), and softer terrain is cleared outright. Getting clear afterwards is not
  /// handled here: the bomb is on the map from the next tick, so the survival step above takes over.
  fn bomb_dig(
    world: &World,
    bot_idx: usize,
    cursor: Cursor,
    danger: &DangerMap,
    params: &BotParams,
    state: &mut BotState,
  ) -> Option<BotAction> {
    if !world.maps.level[cursor].is_passable() {
      return None;
    }
    let inventory = &world.players[bot_idx].inventory;
    let weapon = [
      Equipment::Digger,
      Equipment::SmallBomb,
      Equipment::Dynamite,
      Equipment::BigBomb,
    ]
    .iter()
    .copied()
    .find(|&weapon| inventory[weapon] > 0)?;

    let retreat = Self::retreat_direction(world, cursor, danger, params, weapon);
    if params.verify_retreat && retreat.is_none() {
      return None;
    }

    state.bomb_cooldown = BOMB_COOLDOWN;
    state.clear_plan();
    let mut keys = vec![Key::Bomb];
    if let Some(dir) = retreat {
      keys.push(dir_to_key(dir));
    }
    Some(BotAction {
      select: Some(weapon),
      keys,
    })
  }

  // ---------------------------------------------------------------------------------------------
  // Fallbacks
  // ---------------------------------------------------------------------------------------------

  /// Last resort, and the reason a bot cannot end up alive but motionless (issue #20): move into the
  /// cheapest neighbouring tile there is, digging it if necessary. Only a bot walled in by metal on
  /// all four sides finds nothing here, and that one drops a bomb instead.
  fn unstick(
    world: &World,
    bot_idx: usize,
    cursor: Cursor,
    danger: &DangerMap,
    drilling: u32,
    params: &BotParams,
    rng: &mut ThreadRng,
  ) -> BotAction {
    // Ignore the per-difficulty dig cap here: being stuck is worse than digging something slow.
    let open = BotParams {
      dig_cost_cap: u32::MAX,
      ..*params
    };

    let mut candidates: Vec<(u32, Direction)> = Direction::all()
      .filter_map(|dir| {
        let next = cursor.to(dir);
        if next == cursor {
          return None;
        }
        enter_cost(world, next, drilling, danger, &open, 0).map(|cost| (cost, dir))
      })
      .collect();

    if candidates.is_empty() {
      // Sealed in. A bomb is the only way out; failing that, keep pushing at the wall.
      if world.maps.level[cursor].is_passable() {
        if let Some(weapon) = Self::melee_weapon(world, bot_idx) {
          return BotAction {
            select: Some(weapon),
            keys: vec![Key::Bomb],
          };
        }
      }
      return BotAction::step(world.actors[bot_idx].facing);
    }

    candidates.sort_by_key(|&(cost, _)| cost);
    // Pick randomly among the cheapest few so two bots in the same pocket do not mirror each other
    // forever, and so a bot that keeps bouncing between two tiles eventually breaks the cycle.
    let cheapest = candidates[0].0;
    let choices: Vec<Direction> = candidates
      .iter()
      .filter(|&&(cost, _)| cost <= cheapest + WALK_TICKS)
      .map(|&(_, dir)| dir)
      .collect();
    BotAction::step(*choices.choose(rng).unwrap())
  }

  fn random_open_direction(
    world: &World,
    cursor: Cursor,
    danger: &DangerMap,
    drilling: u32,
    params: &BotParams,
    rng: &mut ThreadRng,
  ) -> Option<Direction> {
    let open: Vec<Direction> = Direction::all()
      .filter(|&dir| {
        let next = cursor.to(dir);
        next != cursor && enter_cost(world, next, drilling, danger, params, 0).is_some()
      })
      .collect();
    open.choose(rng).copied()
  }

  // ---------------------------------------------------------------------------------------------
  // Geometry helpers
  // ---------------------------------------------------------------------------------------------

  /// Check if an actor can enter the tile in direction `dir` without digging.
  pub fn can_step(cursor: Cursor, dir: Direction, level: &crate::world::map::LevelMap) -> bool {
    let next = cursor.to(dir);
    if next == cursor || next.is_on_border() {
      return false;
    }
    let value = level[next];
    value.is_passable() || value.is_sand() || value.is_treasure()
  }

  /// Get direct orthogonal direction towards target if aligned.
  fn dir_towards(cursor: Cursor, target: Cursor) -> Option<Direction> {
    if cursor.row == target.row {
      if target.col > cursor.col {
        Some(Direction::Right)
      } else if target.col < cursor.col {
        Some(Direction::Left)
      } else {
        None
      }
    } else if cursor.col == target.col {
      if target.row > cursor.row {
        Some(Direction::Down)
      } else if target.row < cursor.row {
        Some(Direction::Up)
      } else {
        None
      }
    } else {
      None
    }
  }

  /// Whether every cell strictly between `cursor` and `target` (along their shared row or column) is
  /// passable, i.e. a projectile fired from `cursor` towards `target` would actually reach it rather
  /// than detonating on the first obstruction.
  fn has_clear_line(cursor: Cursor, target: Cursor, world: &World) -> bool {
    let Some(dir) = Self::dir_towards(cursor, target) else {
      return false;
    };
    let mut cell = cursor;
    for _ in 0..MAP_ROWS.max(MAP_COLS) {
      cell = cell.to(dir);
      if cell == target {
        return true;
      }
      if cell.is_on_border() || !world.maps.level[cell].is_passable() {
        return false;
      }
    }
    false
  }
}

// -----------------------------------------------------------------------------------------------
// Free helpers
// -----------------------------------------------------------------------------------------------

#[inline]
fn cell_index(cursor: Cursor) -> usize {
  usize::from(cursor.row) * usize::from(MAP_COLS) + usize::from(cursor.col)
}

#[inline]
fn cell_at(index: usize) -> Cursor {
  Cursor::new(
    (index / usize::from(MAP_COLS)) as u16,
    (index % usize::from(MAP_COLS)) as u16,
  )
}

fn direction_to_adjacent(from: Cursor, to: Cursor) -> Option<Direction> {
  Direction::all().find(|&dir| {
    let next = from.to(dir);
    next != from && next == to
  })
}

/// Tiles away from its own blast a bot has to get after dropping `weapon`, from the explosion
/// patterns in `world::explode` (a small bomb's cross reaches 1 tile, a big bomb's 2, dynamite's 3).
fn blast_clearance(weapon: Equipment) -> u16 {
  match weapon {
    Equipment::SmallBomb | Equipment::Mine | Equipment::Grenade | Equipment::DrillDrone => 1,
    Equipment::BigBomb | Equipment::Barrel | Equipment::ExplosivePlastic => 2,
    Equipment::Dynamite | Equipment::Digger | Equipment::Plastic => 3,
    Equipment::Napalm | Equipment::AtomicBomb => 6,
    _ => 3,
  }
}

/// Whether the actor is part-way through a step: its position is between two tile centers along the
/// axis it is moving on (a tile is 10 units wide and its center is at offset 5, see `animate_actor`).
fn is_between_tiles(actor: &crate::world::actor::ActorComponent) -> bool {
  if !actor.moving {
    return false;
  }
  match actor.facing {
    Direction::Left | Direction::Right => actor.pos.x % 10 != 5,
    Direction::Up | Direction::Down => actor.pos.y % 10 != 5,
  }
}

/// Ticks of hand-digging a tile costs: `interact_map` subtracts `drilling` from the tile's `hits`
/// once per tick for as long as the actor keeps walking into it.
fn dig_ticks(world: &World, cell: Cursor, drilling: u32) -> u32 {
  let hits = world.maps.hits[cell].max(0) as u32;
  hits.div_ceil(drilling.max(1))
}

/// Tiles an actor can walk into right away, used by the escape/retreat searches where digging is out
/// of the question. Treasure counts: `interact_map` collects it in a single tick.
fn is_walkable(world: &World, cell: Cursor) -> bool {
  if cell.is_on_border() {
    return false;
  }
  let value = world.maps.level[cell];
  value.is_passable() || value.is_treasure()
}

/// Cost in ticks of entering `cell`, or `None` if the bot should not go there at all.
///
/// `arrival` is how many ticks away the bot is from starting this step, which is what makes the
/// danger check a timing question rather than a proximity one: a tile that will be hit in 100 ticks
/// is perfectly safe to cross now and lethal to cross in 90 ticks' time. Ignoring that was measurably
/// fatal - a bot that had just dropped a bomb and turned back towards its target would path straight
/// through its own blast, arriving exactly as it went off.
fn enter_cost(
  world: &World,
  cell: Cursor,
  drilling: u32,
  danger: &DangerMap,
  params: &BotParams,
  arrival: u32,
) -> Option<u32> {
  if cell.is_on_border() {
    return None;
  }
  let value = world.maps.level[cell];
  if world.maps.hits[cell] >= 30_000 || value == MapValue::MetalWall {
    return None;
  }

  let mut cost = WALK_TICKS;
  if value.is_passable() {
    // Nothing to clear.
  } else if value.is_treasure()
    || matches!(
      value,
      MapValue::SmallPickaxe
        | MapValue::LargePickaxe
        | MapValue::Drill
        | MapValue::WeaponsCrate
        | MapValue::Medikit
        | MapValue::LifeItem
    )
  {
    // Picked up on contact, one tick.
    cost += 1;
  } else if value.is_sand()
    || value.is_stone_like()
    || value.is_brick_like()
    || matches!(
      value,
      MapValue::LightGravel | MapValue::HeavyGravel | MapValue::Biomass | MapValue::Plastic
    )
  {
    let dig = dig_ticks(world, cell, drilling);
    if dig > params.dig_cost_cap {
      return None;
    }
    cost += dig;
  } else {
    // Bombs, mines, doors, buttons, exits, pushable junk: not something to path through.
    return None;
  }

  if danger.monster_contact(cell) {
    // Walking past a monster costs health, not just time.
    cost += 400;
  }

  let when = danger.at(cell);
  if when != SAFE {
    // We are on this tile from `arrival` until `arrival + cost` (digging keeps us next to it for the
    // whole dig), plus the difficulty's safety margin.
    if u32::from(when) <= arrival + cost + params.escape_margin {
      return None;
    }
    // Clear in time, but still no reason to route past a live bomb when another way exists.
    cost += 200;
  }

  Some(cost)
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


