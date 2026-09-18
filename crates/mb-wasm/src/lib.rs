use mb_core::glyphs::{AnimationPhase, Glyph};
use mb_core::images::{decode_font, decode_spy, DecodedImage};
use mb_core::keys::Key;
use mb_core::options::Options;
use mb_core::sound::SoundEffect;
use mb_core::world::bot::BotDifficulty;
use mb_core::world::equipment::Equipment;
use mb_core::world::map::{LevelMap, MapValue, MAP_COLS};
use mb_core::world::player::PlayerComponent;
use mb_core::world::position::Cursor;
use mb_core::world::{Update, World};
use std::convert::TryInto;

const SCREEN_WIDTH: usize = 640;
const SCREEN_HEIGHT: usize = 480;

static SIKA_SPY_BYTES: &[u8] = include_bytes!("../../../res/minebomb/SIKA.SPY");
static FONTTI_FON_BYTES: &[u8] = include_bytes!("../../../res/minebomb/FONTTI.FON");
static TITLEBE_SPY_BYTES: &[u8] = include_bytes!("../../../res/minebomb/TITLEBE.SPY");

static mut FRAMEBUFFER: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4] = [0; SCREEN_WIDTH * SCREEN_HEIGHT * 4];

#[no_mangle]
pub extern "C" fn mb_render_title() -> u32 {
  let title_img = match decode_spy(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, TITLEBE_SPY_BYTES) {
    Ok(img) => img,
    Err(_) => return 0,
  };

  unsafe {
    for (i, chunk) in title_img.image.chunks_exact(3).enumerate() {
      if i < SCREEN_WIDTH * SCREEN_HEIGHT {
        let d_idx = i * 4;
        FRAMEBUFFER[d_idx] = chunk[0];
        FRAMEBUFFER[d_idx + 1] = chunk[1];
        FRAMEBUFFER[d_idx + 2] = chunk[2];
        FRAMEBUFFER[d_idx + 3] = 255;
      }
    }
  }

  1
}

struct AudioEvent {
  effect_id: i32,
  frequency: i32,
  pan: f32,
}

struct RumbleEvent {
  player_idx: i32,
  intensity: f32,
  duration_ms: f32,
}

struct WebGame {
  players: [PlayerComponent; 4],
  level: LevelMap,
  world: Option<World<'static>>,
  sika: DecodedImage,
  font: Vec<u8>,
  audio_queue: Vec<AudioEvent>,
  rumble_queue: Vec<RumbleEvent>,
  round_start_tick: usize,
  max_round_ticks: usize,
}

static mut GAME: Option<WebGame> = None;

fn equip_bot(player: &mut PlayerComponent, diff: BotDifficulty) {
  match diff {
    BotDifficulty::Easy => {
      player.inventory[Equipment::SmallBomb] = 15;
      player.inventory[Equipment::BigBomb] = 5;
      player.inventory[Equipment::Dynamite] = 3;
      player.inventory[Equipment::SmallPickaxe] = 1;
      player.selection = Equipment::SmallBomb;
      player.cash = 300;
    }
    BotDifficulty::Medium => {
      player.inventory[Equipment::SmallBomb] = 20;
      player.inventory[Equipment::BigBomb] = 8;
      player.inventory[Equipment::Dynamite] = 6;
      player.inventory[Equipment::Mine] = 2;
      player.inventory[Equipment::SmallPickaxe] = 1;
      player.inventory[Equipment::FreezeBomb] = 1;
      player.selection = Equipment::SmallBomb;
      player.cash = 150;
    }
    BotDifficulty::Hard => {
      player.inventory[Equipment::SmallBomb] = 25;
      player.inventory[Equipment::BigBomb] = 10;
      player.inventory[Equipment::Dynamite] = 8;
      player.inventory[Equipment::Mine] = 4;
      player.inventory[Equipment::SmallPickaxe] = 1;
      player.inventory[Equipment::DrillDrone] = 1;
      player.inventory[Equipment::Armor] = 1;
      player.selection = Equipment::SmallBomb;
      player.cash = 50;
    }
  }
}

fn equip_starter_pack(player: &mut PlayerComponent) {
  player.inventory[Equipment::SmallBomb] = 15;
  player.inventory[Equipment::BigBomb] = 5;
  player.inventory[Equipment::Dynamite] = 5;
  player.inventory[Equipment::SmallRadio] = 2;
  player.inventory[Equipment::SmallPickaxe] = 1;
  player.inventory[Equipment::FreezeBomb] = 1;
  player.selection = Equipment::SmallBomb;
  player.cash = 90;
}

#[no_mangle]
pub extern "C" fn mb_init(
  _level_idx: u32,
  diff1: u32,
  diff2: u32,
  diff3: u32,
  diff4: u32,
) -> u32 {
  let sika = match decode_spy(640, 480, SIKA_SPY_BYTES) {
    Ok(img) => img,
    Err(_) => return 0,
  };
  let font = match decode_font(FONTTI_FON_BYTES) {
    Ok(f) => f,
    Err(_) => return 0,
  };

  let level = LevelMap::random_map(10);

  let parse_diff = |d: u32| match d {
    1 => (true, BotDifficulty::Easy),
    2 => (true, BotDifficulty::Medium),
    3 => (true, BotDifficulty::Hard),
    _ => (false, BotDifficulty::Medium),
  };

  let (is_bot1, bot_diff1) = parse_diff(diff1);
  let (is_bot2, bot_diff2) = parse_diff(diff2);
  let (is_bot3, bot_diff3) = parse_diff(diff3);
  let (is_bot4, bot_diff4) = parse_diff(diff4);

  let opts = Options::default();
  let mut players = [
    PlayerComponent::new("PLAYER 1".to_string(), Default::default(), &opts, is_bot1, bot_diff1),
    PlayerComponent::new("PLAYER 2".to_string(), Default::default(), &opts, is_bot2, bot_diff2),
    PlayerComponent::new("PLAYER 3".to_string(), Default::default(), &opts, is_bot3, bot_diff3),
    PlayerComponent::new("PLAYER 4".to_string(), Default::default(), &opts, is_bot4, bot_diff4),
  ];

  for p in players.iter_mut() {
    if p.is_bot {
      equip_bot(p, p.bot_difficulty);
    } else {
      equip_starter_pack(p);
    }
  }

  unsafe {
    FRAMEBUFFER.fill(0);

    let mut game = WebGame {
      players,
      level: level.clone(),
      world: None,
      sika,
      font,
      audio_queue: Vec::with_capacity(32),
      rumble_queue: Vec::with_capacity(16),
      round_start_tick: 0,
      max_round_ticks: 60 * 180,
    };

    game.start_round(level);
    GAME = Some(game);
  }

  1
}

impl WebGame {
  fn start_round(&mut self, level: LevelMap) {
    self.audio_queue.clear();
    self.rumble_queue.clear();
    self.round_start_tick = 0;

    let players_ref: &'static mut [PlayerComponent] = unsafe {
      std::mem::transmute(&mut self.players[..])
    };

    let world = World::create(level, players_ref, false, 100, false);
    self.level = world.maps.level.clone();
    self.world = Some(world);

    self.render_full();
  }

  fn render_full(&self) {
    unsafe {
      FRAMEBUFFER.fill(0);
    }

    for cursor in Cursor::all() {
      let map_val = self.level[cursor];
      self.draw_map_tile(cursor, map_val);
    }

    self.draw_hud();
  }

  fn draw_map_tile(&self, cursor: Cursor, val: MapValue) {
    let dst_x = (cursor.col as i32) * 10;
    let dst_y = 30 + (cursor.row as i32) * 10;

    if val == MapValue::Passage {
      self.fill_rect(dst_x, dst_y, 10, 10, 0, 0, 0);
      return;
    }

    let glyph = Glyph::Map(val);
    let rect = glyph.rect();
    self.blit_sika(rect.x as u32, rect.y as u32, rect.w, rect.h, dst_x, dst_y, false);
  }

  fn fill_rect(&self, x: i32, y: i32, w: u32, h: u32, r: u8, g: u8, b: u8) {
    unsafe {
      for dy in 0..h as i32 {
        let py = y + dy;
        if py < 0 || py >= SCREEN_HEIGHT as i32 {
          continue;
        }
        for dx in 0..w as i32 {
          let px = x + dx;
          if px < 0 || px >= SCREEN_WIDTH as i32 {
            continue;
          }
          let idx = ((py as usize) * SCREEN_WIDTH + (px as usize)) * 4;
          FRAMEBUFFER[idx] = r;
          FRAMEBUFFER[idx + 1] = g;
          FRAMEBUFFER[idx + 2] = b;
          FRAMEBUFFER[idx + 3] = 255;
        }
      }
    }
  }

  fn blit_sika(&self, src_x: u32, src_y: u32, w: u32, h: u32, dst_x: i32, dst_y: i32, transparent: bool) {
    unsafe {
      for dy in 0..h as i32 {
        let py = dst_y + dy;
        if py < 0 || py >= SCREEN_HEIGHT as i32 {
          continue;
        }
        let sy = src_y + dy as u32;
        if sy >= 480 {
          continue;
        }

        for dx in 0..w as i32 {
          let px = dst_x + dx;
          if px < 0 || px >= SCREEN_WIDTH as i32 {
            continue;
          }
          let sx = src_x + dx as u32;
          if sx >= 640 {
            continue;
          }

          let s_idx = ((sy as usize) * 640 + (sx as usize)) * 3;
          let r = self.sika.image[s_idx];
          let g = self.sika.image[s_idx + 1];
          let b = self.sika.image[s_idx + 2];

          if transparent && r == 0 && g == 0 && b == 0 {
            continue;
          }

          let d_idx = ((py as usize) * SCREEN_WIDTH + (px as usize)) * 4;
          FRAMEBUFFER[d_idx] = r;
          FRAMEBUFFER[d_idx + 1] = g;
          FRAMEBUFFER[d_idx + 2] = b;
          FRAMEBUFFER[d_idx + 3] = 255;
        }
      }
    }
  }

  fn draw_text(&self, text: &str, x: i32, y: i32, color_r: u8, color_g: u8, color_b: u8) {
    let mut cur_x = x;
    for ch in text.chars() {
      let b = if ch.is_ascii() { ch as u8 } else { b' ' };
      let col = (b % 16) as u32;
      let row = (b / 16) as u32;
      self.draw_char(col * 8, row * 8, cur_x, y, color_r, color_g, color_b);
      cur_x += 8;
    }
  }

  fn draw_char(&self, src_x: u32, src_y: u32, dst_x: i32, dst_y: i32, cr: u8, cg: u8, cb: u8) {
    unsafe {
      for dy in 0..8 {
        let py = dst_y + dy;
        if py < 0 || py >= SCREEN_HEIGHT as i32 {
          continue;
        }
        for dx in 0..8 {
          let px = dst_x + dx;
          if px < 0 || px >= SCREEN_WIDTH as i32 {
            continue;
          }
          let s_idx = (((src_y + dy as u32) * 128 + (src_x + dx as u32)) * 4) as usize;
          let alpha = self.font[s_idx + 3];
          if alpha > 0 {
            let d_idx = ((py as usize) * SCREEN_WIDTH + (px as usize)) * 4;
            FRAMEBUFFER[d_idx] = cr;
            FRAMEBUFFER[d_idx + 1] = cg;
            FRAMEBUFFER[d_idx + 2] = cb;
            FRAMEBUFFER[d_idx + 3] = 255;
          }
        }
      }
    }
  }

  fn draw_hud(&self) {
    self.fill_rect(0, 0, 640, 30, 20, 20, 24);

    let colors = [
      (240, 200, 50),
      (230, 60, 50),
      (50, 150, 240),
      (50, 220, 90),
    ];

    for idx in 0..4 {
      let p = &self.players[idx];
      let col_x = (idx as i32) * 160;
      let (cr, cg, cb) = colors[idx];

      let tag = if !p.is_bot {
        "P".to_string() + &(idx + 1).to_string()
      } else {
        match p.bot_difficulty {
          BotDifficulty::Easy => "EASY".to_string(),
          BotDifficulty::Medium => "NORM".to_string(),
          BotDifficulty::Hard => "HARD".to_string(),
        }
      };

      let label = format!("{}: ${}", tag, p.cash);
      self.draw_text(&label, col_x + 6, 4, cr, cg, cb);

      let w_tag = match p.selection {
        Equipment::SmallBomb => "BOMB",
        Equipment::BigBomb => "BBOM",
        Equipment::Dynamite => "DYN",
        Equipment::AtomicBomb => "NUKE",
        Equipment::Mine => "MINE",
        Equipment::Grenade => "GREN",
        Equipment::Flamethrower => "FLAM",
        Equipment::Napalm => "NAPM",
        Equipment::BlackHole => "HOLE",
        Equipment::FreezeBomb => "FRZ",
        Equipment::DrillDrone => "DRON",
        Equipment::SmallRadio | Equipment::LargeRadio => "REM",
        Equipment::Plastic | Equipment::ExplosivePlastic => "C4",
        _ => "ITEM",
      };
      let w_cnt = p.inventory[p.selection];
      let w_label = if w_cnt > 0 {
        format!("{}:{}", w_tag, w_cnt)
      } else {
        "[-]".to_string()
      };
      self.draw_text(&w_label, col_x + 84, 4, 210, 210, 210);

      if let Some(ref world) = self.world {
        if idx < world.actors.len() {
          let actor = &world.actors[idx];
          let hp = actor.health.max(0);
          let max_hp = actor.max_health.max(1);
          let bar_w = ((hp as u32 * 140) / max_hp as u32).min(140);
          self.fill_rect(col_x + 6, 16, 140, 6, 50, 50, 50);
          if hp > 0 {
            let bar_color = if hp > max_hp / 2 { (40, 200, 60) } else { (220, 60, 40) };
            self.fill_rect(col_x + 6, 16, bar_w, 6, bar_color.0, bar_color.1, bar_color.2);
          }
        }
      }
    }

    let elapsed = self.round_start_tick;
    let total = self.max_round_ticks;
    let bar_w = ((636 * elapsed) / total).min(636) as u32;
    self.fill_rect(2, 473, 636, 5, 40, 40, 45);
    self.fill_rect(2, 473, 636 - bar_w, 5, 230, 180, 40);
  }
}

#[no_mangle]
pub extern "C" fn mb_step(p1: u32, p2: u32, p3: u32, p4: u32) -> u32 {
  unsafe {
    let game = match GAME.as_mut() {
      Some(g) => g,
      None => return 0,
    };

    game.round_start_tick += 1;

    let (effects, updates, map_snapshots, is_flash, is_end_round, actors_data) = {
      let world = match game.world.as_mut() {
        Some(w) => w,
        None => return 0,
      };

      // Apply human inputs
      let inputs = [p1, p2, p3, p4];
      for (idx, &input_mask) in inputs.iter().enumerate() {
        if game.players[idx].is_bot {
          continue;
        }
        if (input_mask & 1) != 0 { world.player_action(idx, Key::Up); }
        if (input_mask & 2) != 0 { world.player_action(idx, Key::Down); }
        if (input_mask & 4) != 0 { world.player_action(idx, Key::Left); }
        if (input_mask & 8) != 0 { world.player_action(idx, Key::Right); }
        if (input_mask & 16) != 0 { world.player_action(idx, Key::Bomb); }
        if (input_mask & 32) != 0 { world.player_action(idx, Key::Choose); }
        if (input_mask & 64) != 0 { world.player_action(idx, Key::Remote); }
        if (input_mask & 128) != 0 { world.player_action(idx, Key::Stop); }
      }

      world.tick();

      let effects = std::mem::take(&mut world.effects.queue);
      let updates = std::mem::take(&mut world.update.queue);
      let is_flash = world.flash;
      let is_end_round = world.is_end_of_round();

      // Snapshot updated tile values directly from live world map
      let map_snapshots: Vec<(Cursor, MapValue)> = updates
        .iter()
        .filter_map(|u| match u {
          Update::Map(cur) => Some((*cur, world.maps.level[*cur])),
          _ => None,
        })
        .collect();

      // Snapshot actors for rendering
      let actors: Vec<_> = world.actors.iter().map(|a| {
        (a.pos, a.kind, a.facing, a.moving, a.animation)
      }).collect();

      (effects, updates, map_snapshots, is_flash, is_end_round, actors)
    };

    // Process sound effects and rumble
    for req in effects {
      let eff_id = match req.effect {
        SoundEffect::Kili => 0,
        SoundEffect::Picaxe => 1,
        SoundEffect::Explos1 => 2,
        SoundEffect::Explos2 => 3,
        SoundEffect::Explos3 => 4,
        SoundEffect::Explos4 => 5,
        SoundEffect::Explos5 => 6,
        SoundEffect::Aargh => 7,
        SoundEffect::Karjaisu => 8,
        SoundEffect::Pikkupom => 9,
        SoundEffect::Urethan => 10,
        SoundEffect::Applause => 11,
      };

      let pan = ((req.location.col as f32 / MAP_COLS as f32) * 2.0 - 1.0).clamp(-1.0, 1.0);
      game.audio_queue.push(AudioEvent {
        effect_id: eff_id,
        frequency: req.frequency,
        pan,
      });

      match req.effect {
        SoundEffect::Explos1 | SoundEffect::Explos2 | SoundEffect::Explos4 | SoundEffect::Explos5 => {
          let expl_c = req.location.col as f32;
          let expl_r = req.location.row as f32;
          for p_idx in 0..game.players.len() {
            if game.players[p_idx].is_bot || p_idx >= actors_data.len() {
              continue;
            }
            let cur = actors_data[p_idx].0.cursor();
            let dist = ((expl_c - cur.col as f32).powi(2) + (expl_r - cur.row as f32).powi(2)).sqrt();
            if dist < 20.0f32 {
              let intensity = (1.0f32 - (dist / 20.0f32)).max(0.25f32);
              game.rumble_queue.push(RumbleEvent {
                player_idx: p_idx as i32,
                intensity,
                duration_ms: 180.0,
              });
            }
          }
        }
        SoundEffect::Explos3 => {
          for p_idx in 0..game.players.len() {
            if !game.players[p_idx].is_bot {
              game.rumble_queue.push(RumbleEvent {
                player_idx: p_idx as i32,
                intensity: 1.0,
                duration_ms: 400.0,
              });
            }
          }
        }
        _ => {}
      }
    }

    if is_flash {
      FRAMEBUFFER.fill(255);
      for p_idx in 0..game.players.len() {
        if !game.players[p_idx].is_bot {
          game.rumble_queue.push(RumbleEvent {
            player_idx: p_idx as i32,
            intensity: 1.0,
            duration_ms: 300.0,
          });
        }
      }
    } else {
      // First, update live map tiles in both game.level and framebuffer
      for (cursor, val) in map_snapshots {
        game.level[cursor] = val;
        game.draw_map_tile(cursor, val);
      }

      for update in updates {
        match update {
          Update::Map(_) => {} // Already drawn above with live value
          Update::Actor(actor_idx, digging) => {
            if actor_idx < actors_data.len() {
              let (pos, kind, facing, moving, animation) = actors_data[actor_idx];
              let cur = pos.cursor();

              let val = game.level[cur];
              game.draw_map_tile(cur, val);

              let phase = match animation / 5 {
                _ if !moving => AnimationPhase::Phase1,
                0 => AnimationPhase::Phase1,
                1 => AnimationPhase::Phase2,
                2 => AnimationPhase::Phase3,
                3 => AnimationPhase::Phase4,
                4 => AnimationPhase::Phase3,
                _ => AnimationPhase::Phase2,
              };

              let glyph = Glyph::Monster(kind, facing, digging, phase);
              let rect = glyph.rect();
              game.blit_sika(rect.x as u32, rect.y as u32, rect.w, rect.h, pos.x as i32 - 5, pos.y as i32 - 5, true);
            }
          }
          _ => {}
        }
      }
      game.draw_hud();
    }

    if is_end_round {
      if let Some(ref world) = game.world {
        for idx in 0..game.players.len() {
          if idx < world.actors.len() {
            game.players[idx].cash += world.actors[idx].accumulated_cash;
          }
        }
      }
      return 2;
    }

    1
  }
}

#[no_mangle]
pub extern "C" fn mb_get_framebuffer() -> *const u8 {
  unsafe { FRAMEBUFFER.as_ptr() }
}

#[no_mangle]
pub extern "C" fn mb_get_audio_event(out_ptr: *mut i32) -> i32 {
  unsafe {
    let game = match GAME.as_mut() {
      Some(g) => g,
      None => return 0,
    };
    if let Some(event) = game.audio_queue.pop() {
      *out_ptr = event.effect_id;
      *out_ptr.add(1) = event.frequency;
      *out_ptr.add(2) = (event.pan * 100.0) as i32;
      1
    } else {
      0
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_get_rumble_event(out_ptr: *mut f32) -> i32 {
  unsafe {
    let game = match GAME.as_mut() {
      Some(g) => g,
      None => return 0,
    };
    if let Some(event) = game.rumble_queue.pop() {
      *out_ptr = event.player_idx as f32;
      *out_ptr.add(1) = event.intensity;
      *out_ptr.add(2) = event.duration_ms;
      1
    } else {
      0
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_set_player_item(player_idx: u32, item_idx: u32, count: u32) {
  unsafe {
    if let Some(ref mut game) = GAME {
      let p_idx = (player_idx as usize).min(3);
      if let Ok(equipment) = (item_idx as u8).try_into() {
        game.players[p_idx].inventory[equipment] = count as u16;
        if count > 0 && game.players[p_idx].inventory[game.players[p_idx].selection] == 0 {
          game.players[p_idx].selection = equipment;
        }
      }
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_get_player_item(player_idx: u32, item_idx: u32) -> u32 {
  unsafe {
    if let Some(ref game) = GAME {
      let p_idx = (player_idx as usize).min(3);
      if let Ok(equipment) = (item_idx as u8).try_into() {
        return game.players[p_idx].inventory[equipment] as u32;
      }
    }
    0
  }
}

#[no_mangle]
pub extern "C" fn mb_set_player_cash(player_idx: u32, cash: u32) {
  unsafe {
    if let Some(ref mut game) = GAME {
      let p_idx = (player_idx as usize).min(3);
      game.players[p_idx].cash = cash;
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_get_player_cash(player_idx: u32) -> u32 {
  unsafe {
    if let Some(ref game) = GAME {
      let p_idx = (player_idx as usize).min(3);
      return game.players[p_idx].cash;
    }
    0
  }
}

#[no_mangle]
pub extern "C" fn mb_clear_player_items(player_idx: u32) {
  unsafe {
    if let Some(ref mut game) = GAME {
      let p_idx = (player_idx as usize).min(3);
      for eq in Equipment::all_equipment() {
        game.players[p_idx].inventory[eq] = 0;
      }
      game.players[p_idx].selection = Equipment::SmallBomb;
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_equip_starter_pack(player_idx: u32) {
  unsafe {
    if let Some(ref mut game) = GAME {
      let p_idx = (player_idx as usize).min(3);
      equip_starter_pack(&mut game.players[p_idx]);
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_set_bot_difficulty(player_idx: u32, diff: u32) {
  unsafe {
    if let Some(ref mut game) = GAME {
      let idx = (player_idx as usize).min(3);
      match diff {
        0 => {
          game.players[idx].is_bot = false;
        }
        1 => {
          game.players[idx].is_bot = true;
          game.players[idx].bot_difficulty = BotDifficulty::Easy;
        }
        2 => {
          game.players[idx].is_bot = true;
          game.players[idx].bot_difficulty = BotDifficulty::Medium;
        }
        3 => {
          game.players[idx].is_bot = true;
          game.players[idx].bot_difficulty = BotDifficulty::Hard;
        }
        _ => {}
      }
    }
  }
}

static mut RANDOM_SEED: u64 = 0x853c49e6748fea9b;

fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
  for b in buf.iter_mut() {
    unsafe {
      RANDOM_SEED = RANDOM_SEED.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
      *b = (RANDOM_SEED >> 33) as u8;
    }
  }
  Ok(())
}

getrandom::register_custom_getrandom!(custom_getrandom);

#[no_mangle]
pub extern "C" fn mb_seed(seed: u32) {
  unsafe {
    RANDOM_SEED = (seed as u64) ^ 0x853c49e6748fea9b;
  }
}

