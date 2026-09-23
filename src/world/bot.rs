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

/// Which of the shop's exotic items a bot knows how to use. Every one of them can kill the bot that
/// used it, so this is a skill, not just an inventory check: knowing that a freeze bomb freezes its
/// own thrower, or that a crucifix blast runs down the whole row *and* column it was dropped on, is
/// the difference between a weapon and a mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpecialWeapons {
  /// Bombs and nothing else.
  None,
  /// The ones that cannot backfire when used correctly: a clone, a super drill, a flamethrower cone
  /// that only burns forwards, an extinguisher for a bomb there is no room to run from.
  ///
  /// Moving the flamethrower out of this tier and up to Hard-only was tried and measured as no
  /// improvement: equal-equipment Hard vs Medium went 120-107 (shared) to 111-107 (Hard only), both
  /// within noise of even, because Hard was using it too. Kept shared, as the simpler rule.
  Safe,
  /// Everything, including the ones that need the bot to get itself well clear first.
  All,
}

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
  /// Which exotic shop items the bot can use.
  special_weapons: SpecialWeapons,
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
        special_weapons: SpecialWeapons::None,
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
        special_weapons: SpecialWeapons::Safe,
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
        special_weapons: SpecialWeapons::All,
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
      // Nowhere to run. An extinguisher puts the fuse out from up to six tiles away
      // (`activate_extinguisher`), which beats standing in the blast.
      if params.special_weapons != SpecialWeapons::None && world.players[bot_idx].inventory[Equipment::Extinguisher] > 0
      {
        if let Some(dir) = Self::direction_of_nearest_live_bomb(world, cursor) {
          return BotAction {
            select: Some(Equipment::Extinguisher),
            keys: vec![dir_to_key(dir), Key::Bomb],
          };
        }
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

    // 8. EQUIP: a clone to fight and mine alongside us, a super drill for the rock in the way.
    if let Some(action) = Self::utility_action(world, bot_idx, drilling, state.path.first().copied(), params) {
      return action;
    }

    // 9. FOLLOW: walk, dig, or blast the next waypoint open.
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

    // 10. Nothing reachable is worth anything: keep digging rather than stand still.
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

    // The exotic shop items, when this bot knows how to use them and the situation fits.
    if params.special_weapons != SpecialWeapons::None && on_bombable_tile {
      if let Some(action) = Self::special_attack(world, bot_idx, cursor, target, manhattan, danger, params, state) {
        return Some(action);
      }
    }

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

  /// The exotic weapons, in order of how safe they are to use. Each one is gated on the bot being
  /// able to get out of its own way: the freeze bomb freezes its thrower, the crucifix blast runs
  /// down the whole row *and* column it was dropped on, the black hole drags everything within nine
  /// tiles into itself, and the atomic bomb clears twelve. Without these checks they are all just
  /// slower ways for a bot to kill itself - which is what `measure_self_destruction` is for.
  #[allow(clippy::too_many_arguments)]
  fn special_attack(
    world: &World,
    bot_idx: usize,
    cursor: Cursor,
    target: Cursor,
    manhattan: u16,
    danger: &DangerMap,
    params: &BotParams,
    state: &mut BotState,
  ) -> Option<BotAction> {
    let inventory = &world.players[bot_idx].inventory;
    let aligned = cursor.row == target.row || cursor.col == target.col;

    // Flamethrower: a cone that only ever spreads forwards and sideways, never back at the shooter
    // (`FlamethrowerExpansion::can_expand` refuses the reverse direction), so it needs no retreat at
    // all. It also burns straight through rock, which is why it is worth using at close range where
    // a clear line is unlikely.
    if inventory[Equipment::Flamethrower] > 0 && aligned && manhattan <= 7 && state.bomb_cooldown == 0 {
      if let Some(face) = Self::dir_towards(cursor, target) {
        state.bomb_cooldown = RANGED_COOLDOWN;
        return Some(BotAction {
          select: Some(Equipment::Flamethrower),
          keys: vec![dir_to_key(face), Key::Bomb],
        });
      }
    }

    if state.bomb_cooldown > 0 {
      return None;
    }

    // Radio bombs are armed rather than fused: drop one in the path of someone chasing us and set it
    // off later from a safe distance (`remote_is_worth_firing` handles the trigger).
    if params.use_remote && (2..=5).contains(&manhattan) {
      for radio in [Equipment::LargeRadio, Equipment::SmallRadio] {
        if inventory[radio] > 0 {
          state.bomb_cooldown = BOMB_COOLDOWN;
          return Some(BotAction {
            select: Some(radio),
            keys: vec![Key::Bomb],
          });
        }
      }
    }

    if params.special_weapons != SpecialWeapons::All {
      return None;
    }

    // Freeze bomb: 180 ticks of a motionless opponent is worth more than any single blast, but the
    // 5-tile radius catches the thrower too, so this needs real distance before the 90-tick fuse.
    if inventory[Equipment::FreezeBomb] > 0 && manhattan <= 5 {
      if let Some(dir) = Self::retreat_direction(world, cursor, danger, params, Equipment::FreezeBomb) {
        state.bomb_cooldown = BOMB_COOLDOWN;
        state.clear_plan();
        return Some(BotAction {
          select: Some(Equipment::FreezeBomb),
          keys: vec![Key::Bomb, dir_to_key(dir)],
        });
      }
    }

    // Crucifix: the blast travels the full row and column from where it lands, so "far enough away"
    // means off both of them - stepping back along the corridor is not an escape.
    for crucifix in [Equipment::LargeCrucifix, Equipment::SmallCrucifix] {
      let range = if crucifix == Equipment::SmallCrucifix {
        15
      } else {
        MAP_COLS
      };
      if inventory[crucifix] > 0 && aligned && manhattan <= range {
        if let Some(dir) = Self::off_the_cross_direction(world, cursor, danger, params) {
          state.bomb_cooldown = BOMB_COOLDOWN;
          state.clear_plan();
          return Some(BotAction {
            select: Some(crucifix),
            keys: vec![Key::Bomb, dir_to_key(dir)],
          });
        }
      }
    }

    // The two that flatten a neighbourhood. Held back for an enemy that is actually close, since
    // both need a long run afterwards and the bot gives up its position to make it.
    for big in [Equipment::BlackHole, Equipment::AtomicBomb] {
      if inventory[big] > 0 && manhattan <= 6 {
        if let Some(dir) = Self::retreat_direction(world, cursor, danger, params, big) {
          state.bomb_cooldown = BOMB_COOLDOWN;
          state.clear_plan();
          return Some(BotAction {
            select: Some(big),
            keys: vec![Key::Bomb, dir_to_key(dir)],
          });
        }
      }
    }

    None
  }

  /// First step towards a reachable tile that shares neither its row nor its column with `cursor` -
  /// i.e. somewhere a crucifix blast dropped here will not reach.
  fn off_the_cross_direction(
    world: &World,
    cursor: Cursor,
    danger: &DangerMap,
    params: &BotParams,
  ) -> Option<Direction> {
    Self::walk_search(world, cursor, params.escape_steps.max(6), |cell, _| {
      cell.row != cursor.row && cell.col != cursor.col && danger.at(cell) == SAFE
    })
  }

  /// Items that help without being aimed at anyone: a clone that mines and fights for us, and the
  /// super drill for the rock in the way. Neither can hurt the bot, so neither needs a retreat.
  fn utility_action(
    world: &World,
    bot_idx: usize,
    drilling: u32,
    next_tile: Option<Cursor>,
    params: &BotParams,
  ) -> Option<BotAction> {
    if params.special_weapons == SpecialWeapons::None {
      return None;
    }
    let inventory = &world.players[bot_idx].inventory;

    // A clone hunts the other players and banks the gold it digs up into *our* purse (see
    // `interact_map`), and never turns on the player it belongs to (`monster.rs`). Free ally.
    if inventory[Equipment::Clone] > 0 && !Self::has_live_clone(world, bot_idx) {
      return Some(BotAction {
        select: Some(Equipment::Clone),
        keys: vec![Key::Bomb],
      });
    }

    // Super drill: +300 drilling for about 180 ticks. Worth spending on rock that would otherwise
    // cost a big chunk of that, and pointless while one is already running.
    if inventory[Equipment::SuperDrill] > 0 && world.actors[bot_idx].super_drill_count == 0 {
      if let Some(next) = next_tile {
        if dig_ticks(world, next, drilling) > 150 {
          return Some(BotAction {
            select: Some(Equipment::SuperDrill),
            keys: vec![Key::Bomb],
          });
        }
      }
    }

    None
  }

  /// Direction of the nearest live explosive within extinguisher range, along a row or column.
  fn direction_of_nearest_live_bomb(world: &World, cursor: Cursor) -> Option<Direction> {
    for distance in 1..=6i16 {
      for dir in Direction::all() {
        let (delta_row, delta_col) = match dir {
          Direction::Up => (-distance, 0),
          Direction::Down => (distance, 0),
          Direction::Left => (0, -distance),
          Direction::Right => (0, distance),
        };
        let Some(cell) = cursor.offset(delta_row, delta_col) else {
          continue;
        };
        if world.maps.level[cell].is_bomb() && world.maps.timer[cell] > 0 {
          return Some(dir);
        }
      }
    }
    None
  }

  fn has_live_clone(world: &World, bot_idx: usize) -> bool {
    world.actors.iter().any(|actor| {
      !actor.is_dead
        && matches!(actor.kind, crate::world::actor::ActorKind::Clone(player) if player as usize == bot_idx)
    })
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
    Equipment::SmallRadio => 1,
    Equipment::BigBomb | Equipment::Barrel | Equipment::ExplosivePlastic | Equipment::LargeRadio => 2,
    Equipment::Dynamite | Equipment::Digger | Equipment::Plastic => 3,
    // Freezes every actor within 5 tiles, the thrower included (`explode_freeze_bomb`).
    Equipment::FreezeBomb => 5,
    // Pulls everything within 9 tiles into itself for a minute before collapsing (`tick_black_hole`).
    Equipment::BlackHole => 10,
    Equipment::Napalm => 6,
    // A 12-tile radius, and the centre takes double damage (`explode_entity`).
    Equipment::AtomicBomb => 13,
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
  use crate::options::Options;
  use crate::world::map::LevelMap;
  use crate::world::player::PlayerComponent;
  use crate::world::position::Position;

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
    // Ranged options only Medium/Hard use (see `BotParams::use_ranged`) - without these, that
    // differentiator never fires and Easy is compared unfairly favorably.
    p.inventory[Equipment::Grenade] = 5;
    p.inventory[Equipment::DrillDrone] = 3;
    // Same reasoning for the exotic items: `BotParams::special_weapons` decides who can use them,
    // so every difficulty has to be holding them for that to be what the measurement sees.
    p.inventory[Equipment::Flamethrower] = 2;
    p.inventory[Equipment::Clone] = 1;
    p.inventory[Equipment::SuperDrill] = 1;
    p.inventory[Equipment::Extinguisher] = 1;
    p.inventory[Equipment::SmallRadio] = 3;
    p.inventory[Equipment::SmallCrucifix] = 2;
    p.inventory[Equipment::FreezeBomb] = 1;
  }

  fn bot_players(diff_a: BotDifficulty, diff_b: BotDifficulty, options: &Options) -> [PlayerComponent; 2] {
    [
      PlayerComponent::new("A".to_string(), Default::default(), options, true, diff_a),
      PlayerComponent::new("B".to_string(), Default::default(), options, true, diff_b),
    ]
  }

  /// One idle human-shaped slot plus one bot, the shape of a real 2-player game against the computer.
  fn human_and_bot(difficulty: BotDifficulty, options: &Options) -> [PlayerComponent; 2] {
    [
      PlayerComponent::new("Human".to_string(), Default::default(), options, false, difficulty),
      PlayerComponent::new("Bot".to_string(), Default::default(), options, true, difficulty),
    ]
  }

  /// Runs one bot-vs-bot match to a decision (someone dies) or `max_ticks`, and returns the winner's
  /// index (0 or 1), or `None` for a draw/timeout. Both bots get identical equipment, so any skew in
  /// outcomes comes only from `BotDifficulty`, not from one side having better gear.
  fn run_match(level: LevelMap, diff_a: BotDifficulty, diff_b: BotDifficulty, max_ticks: u32) -> Option<usize> {
    let options = Options::default();
    let mut players = bot_players(diff_a, diff_b, &options);
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

  /// Same as `run_match`, but equips both sides via `auto_buy_for_bot` (the function a real game
  /// actually calls for a CPU player in the shop) with `starting_cash` each, instead of the equal
  /// loadout `equip_for_a_fair_fight` gives every difficulty. `auto_buy_for_bot` itself scales
  /// noticeably by difficulty (Hard: 3 armor / 15 bombs / 8 dynamite / 6 grenades / 3 big bombs / 4
  /// mines; Medium: 2 armor / 10 bombs / 5 dynamite / 4 grenades, no big bombs/mines; Easy: 1 armor /
  /// 6 bombs, no dynamite/grenades/big bombs/mines at all), so this measures decision quality *and*
  /// equipment together, the way a real match does.
  fn run_match_with_real_shop_equipment(
    level: LevelMap,
    diff_a: BotDifficulty,
    diff_b: BotDifficulty,
    starting_cash: u32,
    max_ticks: u32,
  ) -> Option<usize> {
    let options = Options::default();
    let prices = crate::menu::shop::Prices::new(options.free_market);
    let mut players = bot_players(diff_a, diff_b, &options);
    for p in players.iter_mut() {
      p.cash = starting_cash;
      crate::menu::shop::auto_buy_for_bot(p, &prices);
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

  /// A solid block of `fill`, so a test can control exactly how much digging stands between the bot
  /// and whatever else it puts on the map.
  fn filled_map(fill: MapValue) -> LevelMap {
    let mut level = LevelMap::empty();
    for cursor in Cursor::all_without_borders() {
      level[cursor] = fill;
    }
    level
  }

  /// Puts the bot (player 1) at `cursor` and clears that tile, whatever the spawn randomization did,
  /// and parks the idle human in the far corner so it is never the more attractive target.
  fn place_bot(world: &mut World, cursor: Cursor) {
    world.maps.level[cursor] = MapValue::Passage;
    world.maps.hits[cursor] = 0;
    world.actors[1].pos = Position::from(cursor);
    world.actors[0].pos = Position::from(Cursor::new(40, 60));
  }

  /// The bot has to *dig* to play this game at all - the gold is inside the rock. This is the single
  /// most important behavior in the file, and the one the first version of this AI could not do:
  /// `can_step` only accepted already-passable tiles, so a treasure walled in by sand was invisible
  /// to every branch of the decision tree.
  #[test]
  fn bot_digs_through_rock_to_reach_buried_treasure() {
    let options = Options::default();
    for difficulty in [BotDifficulty::Easy, BotDifficulty::Medium, BotDifficulty::Hard] {
      let mut reached = 0;
      for _ in 0..5 {
        let mut players = human_and_bot(difficulty, &options);
        let mut world = World::create(filled_map(MapValue::Sand1), &mut players, false, 50, false);
        let start = Cursor::new(20, 20);
        place_bot(&mut world, start);
        let gold = Cursor::new(20, 26);
        world.maps.level[gold] = MapValue::GoldBar;
        world.maps.hits[gold] = 0;

        for _ in 0..1500 {
          world.tick();
          if world.actors[1].accumulated_cash > 0 {
            break;
          }
        }
        if world.actors[1].accumulated_cash > 0 {
          reached += 1;
        }
      }
      assert!(
        reached >= 4,
        "{:?} bot only dug its way to the buried gold in {}/5 runs",
        difficulty,
        reached
      );
    }
  }

  /// A bot standing next to a live bomb has to leave the blast. The blast pattern is exact (fixed
  /// offsets, see `explode_pattern`), so there is a correct answer here and `DangerMap` knows it.
  #[test]
  fn bot_steps_out_of_a_live_blast() {
    let options = Options::default();
    for difficulty in [BotDifficulty::Medium, BotDifficulty::Hard] {
      let mut survived = 0;
      for _ in 0..10 {
        let mut players = human_and_bot(difficulty, &options);
        let mut world = World::create(LevelMap::empty(), &mut players, false, 50, false);
        place_bot(&mut world, Cursor::new(20, 20));

        // A dynamite stick one tile away: radius 3, so a single step sideways is not enough.
        let bomb = Cursor::new(20, 21);
        world.maps.level[bomb] = MapValue::Dynamite1;
        world.maps.timer[bomb] = 80;
        world.maps.hits[bomb] = 20;

        for _ in 0..160 {
          world.tick();
        }
        if !world.actors[1].is_dead {
          survived += 1;
        }
      }
      assert!(
        survived >= 9,
        "{:?} bot survived only {}/10 dynamite sticks dropped next to it",
        difficulty,
        survived
      );
    }
  }

  /// Issue #20: a bot was seen alive and completely motionless for 1800+ ticks. Every fallback in
  /// `decide` now ends in a movement action - there is no "stand still" branch left - so a bot sealed
  /// into a one-tile pocket digs its way out instead of freezing.
  #[test]
  fn walled_in_bot_digs_its_way_out() {
    let options = Options::default();
    for difficulty in [BotDifficulty::Easy, BotDifficulty::Medium, BotDifficulty::Hard] {
      let mut players = human_and_bot(difficulty, &options);
      let mut world = World::create(filled_map(MapValue::LightGravel), &mut players, false, 50, false);
      let start = Cursor::new(20, 20);
      place_bot(&mut world, start);

      let mut escaped = false;
      for _ in 0..900 {
        world.tick();
        if world.actors[1].pos.cursor() != start {
          escaped = true;
          break;
        }
      }
      assert!(escaped, "{:?} bot never dug out of its one-tile pocket", difficulty);
    }
  }

  /// Same shape as `fresh_game_start_bot_moves_from_spawn_random_maps`, but over the whole round and
  /// tracking the longest stretch in which the bot achieved *nothing*: its tile did not change and
  /// none of the surrounding rock lost any `hits`. That is the issue #20 condition. Standing on one
  /// tile is not by itself a fault - hand-digging solid stone without a pickaxe legitimately takes
  /// 2000 ticks, and an earlier version of this test failed intermittently because it counted that
  /// as a freeze.
  #[test]
  fn bot_makes_progress_throughout_the_round() {
    let options = Options::default();
    const TICKS: u32 = 1500;
    const STALL_THRESHOLD: u32 = 300;
    let mut worst: Option<(u32, Cursor)> = None;
    for _ in 0..20u32 {
      let mut level = crate::world::map::LevelMap::random_map(75);
      level.generate_entrances(2);
      let mut players = human_and_bot(BotDifficulty::Medium, &options);
      let mut world = World::create(level, &mut players, false, 50, false);
      // Position plus the toughness of everything around it: digging lowers the second even when the
      // first cannot change yet.
      let progress_marker = |world: &World| {
        let cursor = world.actors[1].pos.cursor();
        let hits: i64 = Direction::all()
          .map(|dir| i64::from(world.maps.hits[cursor.to(dir)]))
          .sum();
        (cursor.row, cursor.col, hits)
      };
      let mut last = progress_marker(&world);
      let mut unchanged_since = 0u32;
      for tick in 1..=TICKS {
        world.tick();
        let marker = progress_marker(&world);
        if marker == last {
          let streak = tick - unchanged_since;
          if worst.is_none_or(|(best, _)| streak > best) {
            worst = Some((streak, world.actors[1].pos.cursor()));
          }
        } else {
          last = marker;
          unchanged_since = tick;
        }
        if world.is_end_of_round() || world.actors[1].is_dead {
          break;
        }
      }
    }
    if let Some((streak, cursor)) = worst {
      assert!(
        streak < STALL_THRESHOLD,
        "bot did nothing at all for {} consecutive ticks at {:?} - neither moving nor digging, which          is the issue #20 freeze",
        streak,
        cursor
      );
    }
  }

  /// How much of the game each difficulty actually plays, on the maps a real New Game generates.
  /// This is the measurement the whole rewrite was aimed at, and it is what "the bots are too dumb"
  /// looked like numerically: the pre-rewrite AI scored dug=4-5, visited=9-11, cash=20-70 on every
  /// difficulty, i.e. it never left its starting cavern and barely touched the map.
  ///
  /// After the rewrite, with the empty starting inventory a fresh round gives (no shop purchases, so
  /// drilling power 1 - a bot that has bought a drill digs an order of magnitude faster):
  ///
  /// | | dug | visited | cash | treasures |
  /// |---|---|---|---|---|
  /// | Easy | 20.1 | 40.4 | 269 | 4.1 |
  /// | Medium | 27.1 | 41.5 | 211 | 3.9 |
  /// | Hard | 49.7 | 59.0 | 635 | 8.6 |
  ///
  /// Medium mining slightly less than Easy is not a regression: it values an enemy higher, so it
  /// spends more of the round hunting - and beats Easy in 83% of resolved matches.
  ///
  /// Ignored by default: 20 rounds x 3 difficulties x 3000 ticks takes a couple of minutes.
  #[test]
  #[ignore = "slow (~2 min): full-round simulation across 60 generated maps"]
  fn measure_bot_activity() {
    let options = Options::default();
    const TRIALS: u32 = 20;
    const TICKS: u32 = 3000;
    for difficulty in [BotDifficulty::Easy, BotDifficulty::Medium, BotDifficulty::Hard] {
      let (mut dug, mut visited, mut cash, mut treasures) = (0i64, 0i64, 0i64, 0i64);
      for _ in 0..TRIALS {
        let mut level = crate::world::map::LevelMap::random_map(75);
        level.generate_entrances(2);
        let mut players = human_and_bot(difficulty, &options);
        let mut world = World::create(level, &mut players, false, 50, false);
        let before = Cursor::all().filter(|c| world.maps.level[*c].is_passable()).count();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..TICKS {
          world.tick();
          let cursor = world.actors[1].pos.cursor();
          seen.insert((cursor.row, cursor.col));
          if world.actors[1].is_dead || world.is_end_of_round() {
            break;
          }
        }
        let after = Cursor::all().filter(|c| world.maps.level[*c].is_passable()).count();
        dug += after as i64 - before as i64;
        visited += seen.len() as i64;
        cash += i64::from(world.actors[1].accumulated_cash);
        treasures += i64::from(world.players[1].stats.treasures_collected);
      }
      let per = |total: i64| total as f64 / f64::from(TRIALS);
      println!(
        "{:?}: dug={:.1} visited={:.1} cash={:.1} treasures={:.1} (avg of {} rounds, {} ticks each)",
        difficulty,
        per(dug),
        per(visited),
        per(cash),
        per(treasures),
        TRIALS,
        TICKS
      );
    }
  }

  /// Win rate per difficulty pairing with *identical* equipment, so the only thing being measured is
  /// decision quality. This is the test two earlier tuning attempts failed to move: with the old AI
  /// every pairing sat at 47-54% (i.e. the three difficulties played the same), and both principled
  /// fixes (shrinking Hard's close-combat range, giving weaker bots a limited vision range) made
  /// things worse rather than better - restricting vision actually made the *restricted* side
  /// stronger, which was the clue that the shared core, not the per-difficulty constants, was the
  /// limiting factor. Hence the rewrite: one competent core, with `BotParams` scaling how well each
  /// difficulty executes it.
  ///
  /// Measured after the rewrite, n=250 (win counts of the matches that resolved): Hard beat Easy
  /// 127-51, Hard beat Medium 111-72, Medium beat Easy 140-28. The Hard/Medium pairing was the last
  /// one to separate - it sat at 98-109 until `situational_aggression` gave Hard a reason to break
  /// off a fight it was losing.
  ///
  /// With the exotic weapons added (`BotParams::special_weapons`) the same measurement reads Hard
  /// 164-63 Easy, Hard 120-107 Medium, Medium 168-36 Easy. Note what moved: both gaps against Easy
  /// widened, while Hard's edge over Medium narrowed to roughly even here - a clone, a super drill
  /// and an extinguisher are simply efficient, and Medium gets all three. The gap that matters for a
  /// player did not narrow; see the shop-equipment test below, where Hard beats Medium 89-15.
  ///
  /// n=250, not 60: at n=60 the noise is larger than the effect (two runs of *identical* code came
  /// back 58% and 38% for the same pairing). Takes a few minutes.
  ///
  /// Needs a real classic map, not `LevelMap::empty()`: on an open map every blast travels
  /// unobstructed across the whole board, which produced simultaneous "both players die together"
  /// chain reactions in ~77% of trials and swamped the signal.
  #[test]
  #[ignore = "needs the original game files: set MB_GAME_DIR or keep them in res/minebomb"]
  fn measure_bot_difficulty_win_rates() {
    const TRIALS: u32 = 250;
    const MAX_TICKS: u32 = 60 * 180;
    let level = real_classic_map("BATTLE.MNE");

    let mut broken = Vec::new();
    for (label, a, b) in [
      ("Hard vs Easy", BotDifficulty::Hard, BotDifficulty::Easy),
      ("Hard vs Medium", BotDifficulty::Hard, BotDifficulty::Medium),
      ("Medium vs Easy", BotDifficulty::Medium, BotDifficulty::Easy),
    ] {
      // Alternate which player index (0 or 1) each side gets: the two spawn corners are not verified
      // symmetric, and difficulty would otherwise be fully confounded with spawn slot.
      let mut a_wins = 0;
      let mut b_wins = 0;
      let mut draws = 0;
      for i in 0..TRIALS {
        let result = if i % 2 == 0 {
          run_match(level.clone(), a, b, MAX_TICKS)
        } else {
          run_match(level.clone(), b, a, MAX_TICKS).map(|winner| 1 - winner)
        };
        match result {
          Some(0) => a_wins += 1,
          Some(1) => b_wins += 1,
          _ => draws += 1,
        }
      }
      println!("{label}: {a:?}={a_wins} {b:?}={b_wins} draws={draws} (of {TRIALS})");
      // Draws are a legitimate result now that the bots survive: two careful, well-armed bots
      // regularly both live out the 3-minute round, and the better-equipped pairings draw most
      // often (Hard vs Medium with shop equipment: 156 of 250). This check is only here to catch a
      // run where essentially nothing resolves, which would make the win counts meaningless.
      if draws * 4 > TRIALS * 3 {
        broken.push(format!("{label}: only {}/{TRIALS} matches resolved", TRIALS - draws));
      }
      if a_wins <= b_wins {
        broken.push(format!("{label}: harder side did not win more ({a_wins} vs {b_wins})"));
      }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
  }

  /// The same pairings with the equipment a real game gives a CPU player (`auto_buy_for_bot` scales
  /// its purchases by difficulty), instead of the equalized loadout above. Both numbers matter: this
  /// one is what a player actually faces, the equalized one isolates the AI itself.
  ///
  /// Measured at n=250 (resolved matches): Hard beat Easy 153-9, Hard beat Medium 89-15, Medium beat
  /// Easy 129-27. Equipment and decision quality compound, so the gaps are much wider here than in the
  /// equalized test - which is the intended shape: picking HARD in the menu should feel like a
  /// different opponent, not like the same bot with a different label.
  #[test]
  #[ignore = "needs the original game files: set MB_GAME_DIR or keep them in res/minebomb"]
  fn measure_bot_difficulty_win_rates_with_real_shop_equipment() {
    const TRIALS: u32 = 250;
    const MAX_TICKS: u32 = 60 * 180;
    // The in-game CASH option's maximum, so each difficulty's shop *priorities* actually diverge
    // instead of everyone being cut off early by a shared budget limit.
    const STARTING_CASH: u32 = 2650;
    let level = real_classic_map("BATTLE.MNE");

    let mut broken = Vec::new();
    for (label, a, b) in [
      ("Hard vs Easy", BotDifficulty::Hard, BotDifficulty::Easy),
      ("Hard vs Medium", BotDifficulty::Hard, BotDifficulty::Medium),
      ("Medium vs Easy", BotDifficulty::Medium, BotDifficulty::Easy),
    ] {
      let mut a_wins = 0;
      let mut b_wins = 0;
      let mut draws = 0;
      for i in 0..TRIALS {
        let result = if i % 2 == 0 {
          run_match_with_real_shop_equipment(level.clone(), a, b, STARTING_CASH, MAX_TICKS)
        } else {
          run_match_with_real_shop_equipment(level.clone(), b, a, STARTING_CASH, MAX_TICKS).map(|winner| 1 - winner)
        };
        match result {
          Some(0) => a_wins += 1,
          Some(1) => b_wins += 1,
          _ => draws += 1,
        }
      }
      println!("{label}: {a:?}={a_wins} {b:?}={b_wins} draws={draws} (of {TRIALS})");
      // Draws are a legitimate result now that the bots survive: two careful, well-armed bots
      // regularly both live out the 3-minute round, and the better-equipped pairings draw most
      // often (Hard vs Medium with shop equipment: 156 of 250). This check is only here to catch a
      // run where essentially nothing resolves, which would make the win counts meaningless.
      if draws * 4 > TRIALS * 3 {
        broken.push(format!("{label}: only {}/{TRIALS} matches resolved", TRIALS - draws));
      }
      if a_wins <= b_wins {
        broken.push(format!("{label}: harder side did not win more ({a_wins} vs {b_wins})"));
      }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
  }

  /// Same shape as a real local game: one human-controlled slot (never sent an action here) plus
  /// Easy/Medium/Hard bots together in one match. Checks the engine handles a mixed 4-actor match
  /// without crashing or hanging, not difficulty balance specifically.
  #[test]
  #[ignore = "needs the original game files: set MB_GAME_DIR or keep them in res/minebomb"]
  fn four_player_match_with_one_human_slot_and_three_bot_difficulties_completes() {
    let level = real_classic_map("BATTLE.MNE");
    let options = Options::default();
    let mut players = [
      PlayerComponent::new(
        "Player".to_string(),
        Default::default(),
        &options,
        false,
        BotDifficulty::Easy,
      ),
      PlayerComponent::new(
        "Bot Easy".to_string(),
        Default::default(),
        &options,
        true,
        BotDifficulty::Easy,
      ),
      PlayerComponent::new(
        "Bot Medium".to_string(),
        Default::default(),
        &options,
        true,
        BotDifficulty::Medium,
      ),
      PlayerComponent::new(
        "Bot Hard".to_string(),
        Default::default(),
        &options,
        true,
        BotDifficulty::Hard,
      ),
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
    assert!(
      !world.actors[0..4].iter().all(|a| a.is_dead),
      "every actor died - likely a mutual chain reaction, not real combat"
    );
  }

  /// Diagnostic (not a regression test): reproduces a fresh match exactly as `play_game` sets one
  /// up - a human slot that never acts, and a bot with the empty starting inventory a new game
  /// actually has - to check whether the bot ever leaves its spawn tile.
  #[test]
  #[ignore = "needs the original game files: set MB_GAME_DIR or keep them in res/minebomb"]
  fn fresh_game_start_bot_moves_from_spawn() {
    let level = real_classic_map("BATTLE.MNE");
    let options = Options::default();
    let mut players = human_and_bot(BotDifficulty::Medium, &options);
    let mut world = World::create(level, &mut players, false, 50, false);
    let spawn_cursor = world.actors[1].pos.cursor();
    for tick in 1..=600u32 {
      world.tick();
      if world.actors[1].pos.cursor() != spawn_cursor {
        println!("moved at tick {}: cursor={:?}", tick, world.actors[1].pos.cursor());
        return;
      }
      if world.is_end_of_round() {
        break;
      }
    }
    panic!("bot never left spawn tile {:?} in 600 ticks", spawn_cursor);
  }

  /// Same as `fresh_game_start_bot_moves_from_spawn`, but using the actual map generation path a
  /// real (non-campaign) New Game uses, across many random maps. Checks whether some unlucky spawn
  /// (e.g. fully enclosed by rock) can trap the bot in a way BATTLE.MNE doesn't.
  #[test]
  fn fresh_game_start_bot_moves_from_spawn_random_maps() {
    let options = Options::default();
    let mut stuck = Vec::new();
    for seed in 0..200u32 {
      let mut level = crate::world::map::LevelMap::random_map(75);
      level.generate_entrances(2);
      let mut players = human_and_bot(BotDifficulty::Medium, &options);
      let mut world = World::create(level, &mut players, false, 50, false);
      let spawn_cursor = world.actors[1].pos.cursor();
      let mut moved = false;
      for _ in 1..=300u32 {
        world.tick();
        if world.actors[1].pos.cursor() != spawn_cursor {
          moved = true;
          break;
        }
        if world.is_end_of_round() {
          break;
        }
      }
      if !moved {
        stuck.push((seed, spawn_cursor, world.actors[1].is_dead));
      }
    }
    if !stuck.is_empty() {
      panic!(
        "bot never moved from spawn in {}/200 random maps: {:?}",
        stuck.len(),
        stuck
      );
    }
  }

  #[test]
  fn danger_map_covers_the_exact_blast_pattern() {
    let options = Options::default();
    let mut players = bot_players(BotDifficulty::Hard, BotDifficulty::Hard, &options);
    let mut world = World::create(LevelMap::empty(), &mut players, false, 50, false);
    let bomb = Cursor::new(20, 20);
    world.maps.level[bomb] = MapValue::SmallBomb1;
    world.maps.timer[bomb] = 42;

    let danger = DangerMap::compute(&world);
    assert_eq!(danger.at(bomb), 42, "the bomb's own tile is in its blast");
    for (delta_row, delta_col) in SMALL_BOMB_PATTERN {
      let cell = bomb.offset(delta_row, delta_col).unwrap();
      assert_eq!(danger.at(cell), 42, "{:?} is inside a small bomb's cross", cell);
    }
    assert_eq!(
      danger.at(bomb.offset(0, 2).unwrap()),
      SAFE,
      "two tiles away is outside a small bomb's cross"
    );
  }

  #[test]
  fn dig_cost_is_measured_in_ticks_of_drilling() {
    let options = Options::default();
    let mut players = bot_players(BotDifficulty::Hard, BotDifficulty::Hard, &options);
    let mut world = World::create(filled_map(MapValue::Stone1), &mut players, false, 50, false);
    let cell = Cursor::new(20, 20);
    // Stone1 is 2000 hits (see `map::hits`), so bare hands need 2000 ticks and a drill 100.
    assert_eq!(dig_ticks(&world, cell, 1), 2000);
    assert_eq!(dig_ticks(&world, cell, 20), 100);
    world.maps.hits[cell] = 0;
    assert_eq!(dig_ticks(&world, cell, 1), 0);
  }

  /// Deaths in a round with nobody fighting back: the opponent slot is a human player that never
  /// acts, so anything that kills the bot was the bot's own doing. This is how the single biggest
  /// flaw in the first version of this AI was found - Hard died in 42 of 150 such rounds, and the
  /// tick-by-tick log of those deaths showed health draining 2-7 points *per tick* with no explosion
  /// anywhere near it. That is monster contact damage: Hard, being the most eager hunter, kept
  /// walking up to monsters it had no reason to fight. With monsters treated as contact hazards
  /// (`DangerMap::contact`) and dropped as targets outside Survival Horde, all three difficulties
  /// now sit at 2 of 150, and Hard's average haul per round went from 781 to 1311.
  ///
  /// Kept as a measurement rather than a pass/fail test: it needs the original game files, and the
  /// number it produces is only meaningful next to the ones above.
  #[test]
  #[ignore = "needs the original game files: set MB_GAME_DIR or keep them in res/minebomb"]
  fn measure_self_destruction() {
    let options = Options::default();
    let prices = crate::menu::shop::Prices::new(options.free_market);
    let level = real_classic_map("BATTLE.MNE");
    const TRIALS: u32 = 150;
    for difficulty in [BotDifficulty::Easy, BotDifficulty::Medium, BotDifficulty::Hard] {
      let mut deaths = 0;
      let mut survivor_health = 0u32;
      let mut cash = 0u32;
      for _ in 0..TRIALS {
        let mut players = human_and_bot(difficulty, &options);
        players[1].cash = 2650;
        crate::menu::shop::auto_buy_for_bot(&mut players[1], &prices);
        let mut world = World::create(level.clone(), &mut players, false, 50, false);
        for _ in 0..3000 {
          world.tick();
          if world.actors[1].is_dead {
            break;
          }
        }
        if world.actors[1].is_dead {
          deaths += 1;
        } else {
          survivor_health += u32::from(world.actors[1].health);
        }
        cash += world.actors[1].accumulated_cash;
      }
      println!(
        "{:?}: died with nobody fighting back {}/{}, surviving health {}, cash {}",
        difficulty,
        deaths,
        TRIALS,
        if deaths < TRIALS {
          survivor_health / (TRIALS - deaths)
        } else {
          0
        },
        cash / TRIALS
      );
    }
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
    assert_eq!(BotController::dir_towards(c1, c1), None);
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
