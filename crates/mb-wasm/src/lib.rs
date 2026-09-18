use mb_core::glyphs::{AnimationPhase, Glyph};
use mb_core::images::{decode_font, decode_spy, Color, DecodedImage};
use mb_core::keys::Key;
use mb_core::options::{Options, WinCondition};
use mb_core::sound::SoundEffect;
use mb_core::world::bot::BotDifficulty;
use mb_core::world::equipment::Equipment;
use mb_core::world::map::{LevelMap, MapValue, MAP_COLS};
use mb_core::world::player::PlayerComponent;
use mb_core::world::position::Cursor;
use mb_core::world::{Update, World};
use std::convert::{TryFrom, TryInto};
use std::time::Duration;

const SCREEN_WIDTH: usize = 640;
const SCREEN_HEIGHT: usize = 480;

static SIKA_SPY_BYTES: &[u8] = include_bytes!("../../../res/minebomb/SIKA.SPY");
static FONTTI_FON_BYTES: &[u8] = include_bytes!("../../../res/minebomb/FONTTI.FON");
static TITLEBE_SPY_BYTES: &[u8] = include_bytes!("../../../res/minebomb/TITLEBE.SPY");
static MAIN3_SPY_BYTES: &[u8] = include_bytes!("../../../res/minebomb/MAIN3.SPY");
static SHOPPIC_SPY_BYTES: &[u8] = include_bytes!("../../../res/minebomb/SHOPPIC.SPY");
static OPTIONS5_SPY_BYTES: &[u8] = include_bytes!("../../../res/minebomb/OPTIONS5.SPY");
static INFO1_SPY_BYTES: &[u8] = include_bytes!("../../../res/minebomb/INFO1.SPY");

static mut FRAMEBUFFER: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4] = [0; SCREEN_WIDTH * SCREEN_HEIGHT * 4];

// Key constants matching web frontend
pub const KEY_UP: u32 = 1;
pub const KEY_DOWN: u32 = 2;
pub const KEY_LEFT: u32 = 3;
pub const KEY_RIGHT: u32 = 4;
pub const KEY_BOMB: u32 = 5;    // Space / Enter (Select, Buy)
pub const KEY_CHOOSE: u32 = 6;  // C / Shift (Sell / refund)
pub const KEY_REMOTE: u32 = 7;  // X / Ctrl
pub const KEY_TAB: u32 = 8;     // Tab / Q / E (Page toggle in shop)
pub const KEY_ESC: u32 = 9;     // Escape
pub const KEY_ANY: u32 = 10;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
  Title = 0,
  MainMenu = 1,
  Options = 2,
  Info = 3,
  Shop = 4,
  Battle = 5,
  RoundEnd = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum GameOption {
  Cash = 0,
  Treasures = 1,
  Rounds = 2,
  Time = 3,
  Players = 4,
  Speed = 5,
  BombDamage = 6,
  Darkness = 7,
  FreeMarket = 8,
  Selling = 9,
  Winner = 10,
  RedefineKeys = 11,
  LoadLevels = 12,
  MainMenu = 13,
}

impl GameOption {
  pub fn from_usize(idx: usize) -> Self {
    match idx {
      0 => GameOption::Cash,
      1 => GameOption::Treasures,
      2 => GameOption::Rounds,
      3 => GameOption::Time,
      4 => GameOption::Players,
      5 => GameOption::Speed,
      6 => GameOption::BombDamage,
      7 => GameOption::Darkness,
      8 => GameOption::FreeMarket,
      9 => GameOption::Selling,
      10 => GameOption::Winner,
      11 => GameOption::RedefineKeys,
      12 => GameOption::LoadLevels,
      _ => GameOption::MainMenu,
    }
  }

  pub fn next(self) -> Self {
    let idx = (self as usize + 1) % 14;
    Self::from_usize(idx)
  }

  pub fn prev(self) -> Self {
    let idx = if self as usize == 0 { 13 } else { self as usize - 1 };
    Self::from_usize(idx)
  }
}

#[derive(Clone, Copy)]
pub struct Prices {
  pub prices: [u32; Equipment::TOTAL],
}

impl Default for Prices {
  fn default() -> Self {
    Self::new()
  }
}

impl Prices {
  pub fn new() -> Prices {
    let mut prices = [0u32; Equipment::TOTAL];
    for eq in Equipment::all_equipment() {
      prices[eq as usize] = eq.base_price();
    }
    Prices { prices }
  }
}

impl std::ops::Index<Equipment> for Prices {
  type Output = u32;
  fn index(&self, index: Equipment) -> &u32 {
    &self.prices[index as usize]
  }
}

#[derive(Clone, Copy)]
pub struct PlayerShopState {
  pub selection: Option<Equipment>,
  pub ready: bool,
  pub page: usize,
}

impl Default for PlayerShopState {
  fn default() -> Self {
    Self::new()
  }
}

impl PlayerShopState {
  pub fn new() -> Self {
    Self {
      selection: Some(Equipment::SmallBomb),
      ready: false,
      page: 0,
    }
  }

  pub fn current_slot(&self) -> usize {
    match (self.page, self.selection) {
      (0, Some(eq)) => (eq as usize).min(26),
      (0, None) => 27,
      (1, Some(eq)) => {
        let idx = eq as usize;
        if (27..=29).contains(&idx) {
          idx - 27
        } else {
          0
        }
      }
      (1, None) => 27,
      _ => 27,
    }
  }
}

pub struct AudioEvent {
  pub effect_id: i32,
  pub frequency: i32,
  pub pan: f32,
}

pub struct RumbleEvent {
  pub player_idx: i32,
  pub intensity: f32,
  pub duration_ms: f32,
}

fn auto_buy_for_bot(player: &mut PlayerComponent, prices: &Prices) {
  let max_armor = match player.bot_difficulty {
    BotDifficulty::Hard => 3,
    BotDifficulty::Medium => 2,
    BotDifficulty::Easy => 1,
  };
  while player.cash >= prices[Equipment::Armor] && player.inventory[Equipment::Armor] < max_armor {
    player.cash -= prices[Equipment::Armor];
    player.inventory[Equipment::Armor] += 1;
  }

  if player.inventory[Equipment::Drill] == 0 && player.cash >= prices[Equipment::Drill] {
    player.cash -= prices[Equipment::Drill];
    player.inventory[Equipment::Drill] += 1;
  } else if player.inventory[Equipment::LargePickaxe] == 0 && player.cash >= prices[Equipment::LargePickaxe] {
    player.cash -= prices[Equipment::LargePickaxe];
    player.inventory[Equipment::LargePickaxe] += 1;
  }

  let max_bombs = match player.bot_difficulty {
    BotDifficulty::Hard => 15,
    BotDifficulty::Medium => 10,
    BotDifficulty::Easy => 6,
  };
  while player.cash >= prices[Equipment::SmallBomb] && player.inventory[Equipment::SmallBomb] < max_bombs {
    player.cash -= prices[Equipment::SmallBomb];
    player.inventory[Equipment::SmallBomb] += 1;
  }

  if player.bot_difficulty != BotDifficulty::Easy {
    let max_dynamite = if player.bot_difficulty == BotDifficulty::Hard { 8 } else { 5 };
    while player.cash >= prices[Equipment::Dynamite] && player.inventory[Equipment::Dynamite] < max_dynamite {
      player.cash -= prices[Equipment::Dynamite];
      player.inventory[Equipment::Dynamite] += 1;
    }
    let max_grenades = if player.bot_difficulty == BotDifficulty::Hard { 6 } else { 4 };
    while player.cash >= prices[Equipment::Grenade] && player.inventory[Equipment::Grenade] < max_grenades {
      player.cash -= prices[Equipment::Grenade];
      player.inventory[Equipment::Grenade] += 1;
    }
  }

  if player.bot_difficulty == BotDifficulty::Hard {
    while player.cash >= prices[Equipment::BigBomb] && player.inventory[Equipment::BigBomb] < 3 {
      player.cash -= prices[Equipment::BigBomb];
      player.inventory[Equipment::BigBomb] += 1;
    }
    while player.cash >= prices[Equipment::Mine] && player.inventory[Equipment::Mine] < 4 {
      player.cash -= prices[Equipment::Mine];
      player.inventory[Equipment::Mine] += 1;
    }
    if player.cash >= prices[Equipment::FreezeBomb] + 300 {
      player.cash -= prices[Equipment::FreezeBomb];
      player.inventory[Equipment::FreezeBomb] += 1;
    }
    if player.cash >= prices[Equipment::DrillDrone] + 300 {
      player.cash -= prices[Equipment::DrillDrone];
      player.inventory[Equipment::DrillDrone] += 1;
    }
    if player.cash >= prices[Equipment::BlackHole] + 500 {
      player.cash -= prices[Equipment::BlackHole];
      player.inventory[Equipment::BlackHole] += 1;
    }
  }

  player.selection = Equipment::SmallBomb;
}

pub fn equip_starter_pack(player: &mut PlayerComponent) {
  player.inventory[Equipment::SmallBomb] = 15;
  player.inventory[Equipment::BigBomb] = 5;
  player.inventory[Equipment::Dynamite] = 5;
  player.inventory[Equipment::SmallRadio] = 2;
  player.inventory[Equipment::SmallPickaxe] = 1;
  player.inventory[Equipment::FreezeBomb] = 1;
  player.selection = Equipment::SmallBomb;
  player.cash = 90;
}

fn preview_pixel(value: MapValue) -> usize {
  if value.is_stone_like() || value == MapValue::Boulder || value == MapValue::Barrel {
    9
  } else if value == MapValue::Diamond || (value >= MapValue::GoldShield && value <= MapValue::GoldCrown) {
    5
  } else if value.is_passable() || value == MapValue::Mine {
    14
  } else if value == MapValue::MetalWall {
    8
  } else if value == MapValue::Biomass {
    4
  } else {
    12
  }
}

pub struct WebGame {
  pub state: AppState,
  pub selected_menu: usize, // 0: New Game, 1: Options, 2: Info, 3: Quit
  pub selected_option: GameOption,
  pub options: Options,
  pub round: u16,
  pub total_rounds: u16,
  pub prices: Prices,
  pub players: [PlayerComponent; 4],
  pub shop_p1: PlayerShopState,
  pub shop_p2: PlayerShopState,
  pub level: LevelMap,
  pub world: Option<World<'static>>,

  // Decoded assets
  pub sika: DecodedImage,
  pub font: Vec<u8>,
  pub title_img: DecodedImage,
  pub main_menu_img: DecodedImage,
  pub shop_img: DecodedImage,
  pub options_img: DecodedImage,
  pub info_img: DecodedImage,

  pub audio_queue: Vec<AudioEvent>,
  pub rumble_queue: Vec<RumbleEvent>,
  pub round_start_tick: usize,
  pub max_round_ticks: usize,
  pub choose_hold_ticks: [u32; 4],
}

static mut GAME: Option<WebGame> = None;

impl WebGame {
  pub fn copy_rgb_to_fb(image: &[u8]) {
    unsafe {
      for (i, chunk) in image.chunks_exact(3).enumerate() {
        if i < SCREEN_WIDTH * SCREEN_HEIGHT {
          let d_idx = i * 4;
          FRAMEBUFFER[d_idx] = chunk[0];
          FRAMEBUFFER[d_idx + 1] = chunk[1];
          FRAMEBUFFER[d_idx + 2] = chunk[2];
          FRAMEBUFFER[d_idx + 3] = 255;
        }
      }
    }
  }

  pub fn set_pixel(&self, px: i32, py: i32, r: u8, g: u8, b: u8) {
    if px >= 0 && px < SCREEN_WIDTH as i32 && py >= 0 && py < SCREEN_HEIGHT as i32 {
      let idx = ((py as usize) * SCREEN_WIDTH + (px as usize)) * 4;
      unsafe {
        FRAMEBUFFER[idx] = r;
        FRAMEBUFFER[idx + 1] = g;
        FRAMEBUFFER[idx + 2] = b;
        FRAMEBUFFER[idx + 3] = 255;
      }
    }
  }

  pub fn draw_vline(&self, x: i32, y1: i32, y2: i32, r: u8, g: u8, b: u8) {
    let start_y = y1.min(y2);
    let end_y = y1.max(y2);
    for py in start_y..=end_y {
      self.set_pixel(x, py, r, g, b);
    }
  }

  pub fn fill_rect(&self, x: i32, y: i32, w: u32, h: u32, r: u8, g: u8, b: u8) {
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

  pub fn draw_text(&self, text: &str, x: i32, y: i32, cr: u8, cg: u8, cb: u8) {
    let mut cur_x = x;
    for ch in text.chars() {
      let b = if ch.is_ascii() { ch as u8 } else { b' ' };
      let col = (b % 16) as u32;
      let row = (b / 16) as u32;
      self.draw_char(col * 8, row * 8, cur_x, y, cr, cg, cb);
      cur_x += 8;
    }
  }

  pub fn draw_char(&self, src_x: u32, src_y: u32, dst_x: i32, dst_y: i32, cr: u8, cg: u8, cb: u8) {
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

  pub fn blit_sika(&self, src_x: u32, src_y: u32, w: u32, h: u32, dst_x: i32, dst_y: i32, transparent: bool) {
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

  pub fn blit_glyph(&self, x: i32, y: i32, glyph: Glyph) {
    let rect = glyph.rect();
    self.blit_sika(rect.x as u32, rect.y as u32, rect.w, rect.h, x, y, true);
  }

  pub fn render_current_state(&mut self) {
    match self.state {
      AppState::Title => {
        Self::copy_rgb_to_fb(&self.title_img.image);
        let p = &self.title_img.palette;
        self.draw_text("PRESS ANY KEY OR ENTER TO START", 184, 440, p[1].r, p[1].g, p[1].b);
      }
      AppState::MainMenu => {
        Self::copy_rgb_to_fb(&self.main_menu_img.image);
        let p = &self.main_menu_img.palette;

        let reg = "MINE BOMBERS RELOADED";
        let pos = ((26 - reg.len()) * 4 + 254) as i32;
        self.draw_text(reg, pos - 1, 437, p[10].r, p[10].g, p[10].b);
        self.draw_text(reg, pos + 1, 437, p[8].r, p[8].g, p[8].b);
        self.draw_text(reg, pos, 437, p[0].r, p[0].g, p[0].b);

        self.draw_text("UP/DOWN: MOVE   ENTER: SELECT", 180, 460, p[8].r, p[8].g, p[8].b);

        let shovel_y = 136 + 48 * (self.selected_menu as i32);
        self.blit_glyph(222, shovel_y, Glyph::ShovelPointer);
      }
      AppState::Options => {
        Self::copy_rgb_to_fb(&self.options_img.image);
        let p = &self.options_img.palette;

        const MENU_ITEM_X: i32 = 192;
        const MENU_ITEM_Y: i32 = 96;
        const ITEM_HEIGHT: i32 = 24;

        // Draw cursor arrow pointer
        let cursor_y = (self.selected_option as i32) * ITEM_HEIGHT + MENU_ITEM_Y + 6;
        self.blit_glyph(MENU_ITEM_X + 25, cursor_y, Glyph::ArrowPointer);

        // Render option values
        for i in 0..14 {
          let opt = GameOption::from_usize(i);
          let opt_y = (i as i32) * ITEM_HEIGHT;

          if i <= 6 {
            // Value bar
            self.fill_rect(MENU_ITEM_X + 142, MENU_ITEM_Y + 5 + opt_y, 166, 13, 0, 0, 0);
            let bar_w = match opt {
              GameOption::Cash => (u64::from(self.options.cash) * 165 / 2650) as u32,
              GameOption::Treasures => (u64::from(self.options.treasures) * 165 / 75) as u32,
              GameOption::Rounds => (u64::from(self.options.rounds) * 165 / 55) as u32,
              GameOption::Time => (self.options.round_time.as_secs() * 165 / 1359) as u32,
              GameOption::Players => (u64::from(self.options.players.saturating_sub(1)) * 55) as u32,
              GameOption::Speed => {
                let spd = 100 - 3 * u64::from(self.options.speed);
                (spd * 165 / 100) as u32
              }
              GameOption::BombDamage => (u64::from(self.options.bomb_damage) * 165 / 100) as u32,
              _ => 0,
            };
            self.fill_rect(MENU_ITEM_X + 142, MENU_ITEM_Y + 5 + opt_y, (bar_w + 1).min(166), 13, p[1].r, p[1].g, p[1].b);

            let txt = match opt {
              GameOption::Cash => Some(format!("{}", self.options.cash)),
              GameOption::Treasures => Some(format!("{}", self.options.treasures)),
              GameOption::Rounds => Some(format!("{}", self.options.rounds)),
              GameOption::Time => {
                let s = self.options.round_time.as_secs();
                Some(format!("{}:{:02} min", s / 60, s % 60))
              }
              GameOption::Players => Some(format!(" {}", self.options.players)),
              GameOption::Speed => Some(format!(" {}%", 100 - 3 * self.options.speed)),
              GameOption::BombDamage => Some(format!(" {}%", self.options.bomb_damage)),
              _ => None,
            };
            if let Some(t) = txt {
              self.draw_text(&t, MENU_ITEM_X + 208, MENU_ITEM_Y + 7 + opt_y, p[8].r, p[8].g, p[8].b);
            }
          } else if i >= 7 && i <= 10 {
            // Radio buttons
            let enabled = match opt {
              GameOption::Darkness => self.options.darkness,
              GameOption::FreeMarket => self.options.free_market,
              GameOption::Selling => self.options.selling,
              GameOption::Winner => self.options.win == WinCondition::ByMoney,
              _ => false,
            };
            self.blit_glyph(MENU_ITEM_X + 185, MENU_ITEM_Y + 5 + opt_y, Glyph::RadioButton(enabled));
            self.blit_glyph(MENU_ITEM_X + 251, MENU_ITEM_Y + 5 + opt_y, Glyph::RadioButton(!enabled));
          }
        }

        self.draw_text("ARROWS: ADJUST   ENTER: SELECT   ESC: MAIN MENU", 140, 455, p[8].r, p[8].g, p[8].b);
      }
      AppState::Info => {
        Self::copy_rgb_to_fb(&self.info_img.image);
        let p = &self.info_img.palette;
        self.draw_text("PRESS ANY KEY OR ESCAPE TO RETURN", 180, 455, p[1].r, p[1].g, p[1].b);
      }
      AppState::Shop => {
        Self::copy_rgb_to_fb(&self.shop_img.image);
        let p = &self.shop_img.palette;

        // Remaining rounds at (306, 120)
        let rem_str = format!("{}", self.total_rounds.saturating_sub(self.round) + 1);
        self.draw_text(&rem_str, 306, 120, p[1].r, p[1].g, p[1].b);

        // Minimap preview at (288, 51, 64, 45)
        for row in 0..45 {
          for col in 0..64 {
            let val = self.level[row as u16][col as u16];
            let color_idx = preview_pixel(val);
            let c = p[color_idx];
            self.set_pixel(288 + col as i32, 51 + row as i32, c.r, c.g, c.b);
          }
        }

        // Left Player (P1) stats
        let p1 = &self.players[0];
        let power1 = 1 + p1.initial_drilling_power();
        self.fill_rect(35, 30, 7 * 8, 8, 0, 0, 0);
        self.fill_rect(35, 44, 7 * 8, 8, 0, 0, 0);
        self.fill_rect(35, 58, 7 * 8, 8, 0, 0, 0);
        self.draw_text(&p1.stats.name, 35, 16, p[1].r, p[1].g, p[1].b);
        self.draw_text(&power1.to_string(), 35, 30, p[3].r, p[3].g, p[3].b);
        self.draw_text(&p1.cash.to_string(), 35, 44, p[5].r, p[5].g, p[5].b);
        if let Some(item) = self.shop_p1.selection {
          let cnt = p1.inventory[item].to_string();
          self.draw_text(&cnt, 35, 58, p[1].r, p[1].g, p[1].b);
        }

        // Right Player (Bot 1) stats
        let p2 = &self.players[1];
        let power2 = 1 + p2.initial_drilling_power();
        self.fill_rect(455, 30, 7 * 8, 8, 0, 0, 0);
        self.fill_rect(455, 44, 7 * 8, 8, 0, 0, 0);
        self.fill_rect(455, 58, 7 * 8, 8, 0, 0, 0);
        let name2 = if p2.is_bot {
          match p2.bot_difficulty {
            BotDifficulty::Easy => "CPU (EASY)".to_string(),
            BotDifficulty::Medium => "CPU (NORM)".to_string(),
            BotDifficulty::Hard => "CPU (HARD)".to_string(),
          }
        } else {
          p2.stats.name.clone()
        };
        self.draw_text(&name2, 455, 16, p[1].r, p[1].g, p[1].b);
        self.draw_text(&power2.to_string(), 455, 30, p[3].r, p[3].g, p[3].b);
        self.draw_text(&p2.cash.to_string(), 455, 44, p[5].r, p[5].g, p[5].b);
        if let Some(item) = self.shop_p2.selection {
          let cnt = p2.inventory[item].to_string();
          self.draw_text(&cnt, 455, 58, p[1].r, p[1].g, p[1].b);
        }

        // Draw Slots for P1 (left: offset_x = 0)
        let page1 = self.shop_p1.page;
        let selected1 = self.shop_p1.current_slot();
        if page1 == 0 {
          for slot in 0..=26 {
            let eq = Equipment::try_from(slot as u8).ok();
            self.render_shop_slot(0, slot, eq, slot == selected1, p1, p);
          }
        } else {
          for slot in 0..3 {
            let eq = Equipment::try_from((27 + slot) as u8).ok();
            self.render_shop_slot(0, slot, eq, slot == selected1, p1, p);
          }
          for slot in 3..27 {
            self.render_empty_slot(0, slot);
          }
        }
        self.render_shop_slot(0, 27, None, selected1 == 27, p1, p);

        // Page banner P1
        self.fill_rect(80, 442, 160, 14, 0, 0, 0);
        let banner1 = if page1 == 0 { "PAGE 1/2 [TAB]" } else { "PAGE 2/2 [TAB]" };
        self.draw_text(banner1, 96, 444, p[1].r, p[1].g, p[1].b);

        // Draw Slots for P2 (right: offset_x = 320)
        let page2 = self.shop_p2.page;
        let selected2 = self.shop_p2.current_slot();
        if page2 == 0 {
          for slot in 0..=26 {
            let eq = Equipment::try_from(slot as u8).ok();
            self.render_shop_slot(320, slot, eq, slot == selected2, p2, p);
          }
        } else {
          for slot in 0..3 {
            let eq = Equipment::try_from((27 + slot) as u8).ok();
            self.render_shop_slot(320, slot, eq, slot == selected2, p2, p);
          }
          for slot in 3..27 {
            self.render_empty_slot(320, slot);
          }
        }
        self.render_shop_slot(320, 27, None, selected2 == 27, p2, p);

        // Page banner P2
        self.fill_rect(400, 442, 160, 14, 0, 0, 0);
        let banner2 = if page2 == 0 { "PAGE 1/2 [TAB]" } else { "PAGE 2/2 [TAB]" };
        self.draw_text(banner2, 416, 444, p[1].r, p[1].g, p[1].b);
      }
      AppState::Battle => {
        self.render_full();
      }
      AppState::RoundEnd => {
        self.fill_rect(140, 200, 360, 80, 20, 20, 30);
        let cash = self.players[0].cash;
        self.draw_text("ROUND FINISHED!", 240, 215, 255, 220, 50);
        let c_str = format!("CURRENT BANK: ${}", cash);
        self.draw_text(&c_str, 220, 235, 100, 240, 120);
        self.draw_text("PRESS ANY KEY TO VISIT SHOP", 200, 255, 220, 220, 220);
      }
    }
  }

  fn render_shop_slot(
    &self,
    offset_x: i32,
    slot_index: usize,
    slot: Option<Equipment>,
    is_selected: bool,
    player: &PlayerComponent,
    palette: &[Color; 16],
  ) {
    let col = (slot_index % 4) as i32;
    let row = (slot_index / 4) as i32;

    let pos_x = col * 64 + 32 + offset_x;
    let pos_y = row * 48 + 96;

    // Slot outline
    self.blit_glyph(pos_x, pos_y, Glyph::ShopSlot(is_selected));

    // Item count bar gauge
    let item_count = slot.map(|item| player.inventory[item] as i32).unwrap_or(0);
    if item_count != 0 {
      let gx = col * 64 + 88 + offset_x;
      let gy = row * 48 + 99;
      let delta = 40 - ((item_count * 2).min(40));
      for (idx, color_idx) in [14, 13, 12, 11, 7].iter().copied().enumerate() {
        let c = palette[color_idx];
        self.draw_vline(gx + idx as i32, gy + delta, gy + 41, c.r, c.g, c.b);
      }
    }

    // Item glyph
    let ix = col * 64 + 49 + offset_x;
    let iy = row * 48 + 99;
    let glyph = slot.map(Glyph::Selection).unwrap_or(Glyph::Ready);
    self.blit_glyph(ix, iy, glyph);

    // Price text
    let tx = col * 64 + 44 + offset_x;
    let ty = row * 48 + 132;
    let text = slot
      .map(|s| format!("{}$", self.prices[s]))
      .unwrap_or_else(|| "LEAVE".to_string());
    self.draw_text(&text, tx, ty, palette[5].r, palette[5].g, palette[5].b);
  }

  fn render_empty_slot(&self, offset_x: i32, slot: usize) {
    let col = (slot % 4) as i32;
    let row = (slot / 4) as i32;
    let pos_x = col * 64 + 32 + offset_x;
    let pos_y = row * 48 + 96;
    self.blit_glyph(pos_x, pos_y, Glyph::ShopSlot(false));
  }

  pub fn handle_key(&mut self, key: u32) {
    match self.state {
      AppState::Title => {
        self.state = AppState::MainMenu;
        self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
        self.render_current_state();
      }
      AppState::MainMenu => match key {
        KEY_UP => {
          self.selected_menu = if self.selected_menu == 0 { 3 } else { self.selected_menu - 1 };
          self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
          self.render_current_state();
        }
        KEY_DOWN => {
          self.selected_menu = (self.selected_menu + 1) % 4;
          self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
          self.render_current_state();
        }
        KEY_BOMB => match self.selected_menu {
          0 => {
            // NEW GAME: Start match using current configured options, open authentic DOS Shop
            self.round = 1;
            self.total_rounds = self.options.rounds;
            self.level = LevelMap::random_map(10);
            self.world = None;
            for eq in Equipment::all_equipment() {
              self.players[0].inventory[eq] = 0;
            }
            self.players[0].cash = self.options.cash as u32;
            self.players[0].selection = Equipment::SmallBomb;
            for p in self.players[1..].iter_mut() {
              auto_buy_for_bot(p, &self.prices);
            }
            self.shop_p1 = PlayerShopState::new();
            self.shop_p2 = PlayerShopState::new();
            self.shop_p2.ready = true;
            self.state = AppState::Shop;
            self.audio_queue.push(AudioEvent { effect_id: 0, frequency: 11000, pan: 0.0 });
            self.render_current_state();
          }
          1 => {
            // OPTIONS: Open authentic Options menu screen
            self.selected_option = GameOption::MainMenu;
            self.state = AppState::Options;
            self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
            self.render_current_state();
          }
          2 => {
            // INFO: Open authentic Info screen
            self.state = AppState::Info;
            self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
            self.render_current_state();
          }
          3 => {
            // QUIT: Return to Title Screen
            self.state = AppState::Title;
            self.render_current_state();
          }
          _ => {}
        },
        _ => {}
      },
      AppState::Options => match key {
        KEY_UP => {
          self.selected_option = self.selected_option.prev();
          self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
          self.render_current_state();
        }
        KEY_DOWN => {
          self.selected_option = self.selected_option.next();
          self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
          self.render_current_state();
        }
        KEY_LEFT => {
          match self.selected_option {
            GameOption::Cash => {
              self.options.cash = if self.options.cash >= 100 { self.options.cash - 100 } else { 0 };
            }
            GameOption::Treasures => {
              if self.options.treasures > 0 { self.options.treasures -= 1; }
            }
            GameOption::Rounds => {
              if self.options.rounds > 1 { self.options.rounds -= 1; }
            }
            GameOption::Time => {
              let s = self.options.round_time.as_secs().saturating_sub(15);
              self.options.round_time = Duration::from_secs(s);
            }
            GameOption::Players => {
              if self.options.players > 1 { self.options.players -= 1; }
            }
            GameOption::Speed => {
              if self.options.speed < 33 { self.options.speed += 1; }
            }
            GameOption::BombDamage => {
              if self.options.bomb_damage >= 5 { self.options.bomb_damage -= 5; }
            }
            GameOption::Darkness => { self.options.darkness = !self.options.darkness; }
            GameOption::FreeMarket => { self.options.free_market = !self.options.free_market; }
            GameOption::Selling => { self.options.selling = !self.options.selling; }
            GameOption::Winner => {
              self.options.win = match self.options.win {
                WinCondition::ByMoney => WinCondition::ByWins,
                WinCondition::ByWins => WinCondition::ByMoney,
              };
            }
            _ => {}
          }
          self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
          self.render_current_state();
        }
        KEY_RIGHT => {
          match self.selected_option {
            GameOption::Cash => {
              if self.options.cash <= 2550 { self.options.cash += 100; }
            }
            GameOption::Treasures => {
              if self.options.treasures < 75 { self.options.treasures += 1; }
            }
            GameOption::Rounds => {
              if self.options.rounds < 55 { self.options.rounds += 1; }
            }
            GameOption::Time => {
              let s = (self.options.round_time.as_secs() + 15).min(1359);
              self.options.round_time = Duration::from_secs(s);
            }
            GameOption::Players => {
              if self.options.players < 4 { self.options.players += 1; }
            }
            GameOption::Speed => {
              if self.options.speed > 0 { self.options.speed -= 1; }
            }
            GameOption::BombDamage => {
              if self.options.bomb_damage <= 95 { self.options.bomb_damage += 5; }
            }
            GameOption::Darkness => { self.options.darkness = !self.options.darkness; }
            GameOption::FreeMarket => { self.options.free_market = !self.options.free_market; }
            GameOption::Selling => { self.options.selling = !self.options.selling; }
            GameOption::Winner => {
              self.options.win = match self.options.win {
                WinCondition::ByMoney => WinCondition::ByWins,
                WinCondition::ByWins => WinCondition::ByMoney,
              };
            }
            _ => {}
          }
          self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
          self.render_current_state();
        }
        KEY_BOMB => match self.selected_option {
          GameOption::MainMenu => {
            self.state = AppState::MainMenu;
            self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
            self.render_current_state();
          }
          GameOption::Darkness => {
            self.options.darkness = !self.options.darkness;
            self.render_current_state();
          }
          GameOption::FreeMarket => {
            self.options.free_market = !self.options.free_market;
            self.render_current_state();
          }
          GameOption::Selling => {
            self.options.selling = !self.options.selling;
            self.render_current_state();
          }
          GameOption::Winner => {
            self.options.win = match self.options.win {
              WinCondition::ByMoney => WinCondition::ByWins,
              WinCondition::ByWins => WinCondition::ByMoney,
            };
            self.render_current_state();
          }
          _ => {}
        },
        KEY_ESC => {
          self.state = AppState::MainMenu;
          self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
          self.render_current_state();
        }
        _ => {}
      },
      AppState::Info => {
        // Any key or escape returns to MainMenu
        self.state = AppState::MainMenu;
        self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
        self.render_current_state();
      }
      AppState::Shop => {
        if key == KEY_ESC {
          self.state = AppState::MainMenu;
          self.render_current_state();
          return;
        }

        if key == KEY_TAB {
          self.shop_p1.page = 1 - self.shop_p1.page;
          if self.shop_p1.selection.is_some() {
            self.shop_p1.selection = if self.shop_p1.page == 1 {
              Some(Equipment::BlackHole)
            } else {
              Some(Equipment::SmallBomb)
            };
          }
          self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
          self.render_current_state();
          return;
        }

        let last_slot = self.shop_p1.current_slot();
        match key {
          KEY_RIGHT => {
            if self.shop_p1.page == 0 {
              let new_slot = (last_slot + 1).min(27);
              self.shop_p1.selection = if new_slot < 27 {
                Equipment::try_from(new_slot as u8).ok()
              } else {
                None
              };
            } else {
              let new_slot = match last_slot {
                0 => 1,
                1 => 2,
                _ => 27,
              };
              self.shop_p1.selection = if new_slot < 3 {
                Equipment::try_from((27 + new_slot) as u8).ok()
              } else {
                None
              };
            }
            self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
            self.render_current_state();
          }
          KEY_LEFT => {
            if self.shop_p1.page == 0 {
              let new_slot = last_slot.saturating_sub(1);
              self.shop_p1.selection = if new_slot < 27 {
                Equipment::try_from(new_slot as u8).ok()
              } else {
                None
              };
            } else {
              let new_slot = match last_slot {
                27 => 2,
                2 => 1,
                _ => 0,
              };
              self.shop_p1.selection = if new_slot < 3 {
                Equipment::try_from((27 + new_slot) as u8).ok()
              } else {
                None
              };
            }
            self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
            self.render_current_state();
          }
          KEY_DOWN => {
            if self.shop_p1.page == 0 {
              let new_slot = (last_slot + 4).min(27);
              self.shop_p1.selection = if new_slot < 27 {
                Equipment::try_from(new_slot as u8).ok()
              } else {
                None
              };
            } else {
              self.shop_p1.selection = None;
            }
            self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
            self.render_current_state();
          }
          KEY_UP => {
            if self.shop_p1.page == 0 {
              let new_slot = if last_slot >= 4 { last_slot - 4 } else { last_slot };
              self.shop_p1.selection = if new_slot < 27 {
                Equipment::try_from(new_slot as u8).ok()
              } else {
                None
              };
            } else {
              let new_slot = if last_slot == 27 { 2 } else { last_slot };
              self.shop_p1.selection = if new_slot < 3 {
                Equipment::try_from((27 + new_slot) as u8).ok()
              } else {
                None
              };
            }
            self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
            self.render_current_state();
          }
          KEY_BOMB => {
            if let Some(selection) = self.shop_p1.selection {
              let price = self.prices[selection];
              if self.players[0].cash >= price {
                self.players[0].cash -= price;
                self.players[0].inventory[selection] += 1;
                self.audio_queue.push(AudioEvent { effect_id: 0, frequency: 11000, pan: 0.0 });
                self.render_current_state();
              }
            } else {
              // LEAVE selected: Enter arena!
              self.shop_p1.ready = true;
              let level = self.level.clone();
              self.start_round(level);
              self.state = AppState::Battle;
              self.audio_queue.push(AudioEvent { effect_id: 0, frequency: 11000, pan: 0.0 });
              self.render_full();
            }
          }
          KEY_CHOOSE => {
            if let Some(selection) = self.shop_p1.selection {
              if self.players[0].inventory[selection] > 0 {
                let price = self.prices[selection];
                let refund = (7 * price + 5) / 10;
                self.players[0].cash += refund;
                self.players[0].inventory[selection] -= 1;
                self.audio_queue.push(AudioEvent { effect_id: 1, frequency: 11000, pan: 0.0 });
                self.render_current_state();
              }
            }
          }
          _ => {}
        }
      }
      AppState::RoundEnd => {
        // Transition back to Shop for next round
        self.state = AppState::Shop;
        self.shop_p1.ready = false;
        self.render_current_state();
      }
      AppState::Battle => {
        if key == KEY_ESC {
          self.state = AppState::MainMenu;
          self.render_current_state();
        }
      }
    }
  }

  fn start_round(&mut self, level: LevelMap) {
    self.audio_queue.clear();
    self.rumble_queue.clear();
    self.round_start_tick = 0;
    self.choose_hold_ticks = [0; 4];

    let players_ref: &'static mut [PlayerComponent] = unsafe {
      std::mem::transmute(&mut self.players[..])
    };

    let world = World::create(level, players_ref, self.options.darkness, self.options.bomb_damage, false);
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

    let remaining = total.saturating_sub(elapsed);
    if let Some(ref world) = self.world {
      if world.sudden_death_active {
        // Red sudden death bar!
        self.fill_rect(2, 473, 636 - bar_w, 5, 240, 40, 30);
        let ring_str = format!("SUDDEN DEATH! RING {}", world.sudden_death_ring);
        self.draw_text(&ring_str, 224, 464, 255, 60, 50);
      } else if world.sudden_death_warning {
        // Flashing yellow/red warning bar
        let flash_color = if (elapsed / 15) % 2 == 0 { (255, 40, 40) } else { (255, 210, 30) };
        self.fill_rect(2, 473, 636 - bar_w, 5, flash_color.0, flash_color.1, flash_color.2);
        let warn_sec = (remaining / 60) + 1;
        let warn_str = format!("!! COLLAPSE IN {}S !!", warn_sec);
        self.draw_text(&warn_str, 224, 464, flash_color.0, flash_color.1, flash_color.2);
      } else {
        self.fill_rect(2, 473, 636 - bar_w, 5, 230, 180, 40);
      }
    } else {
      self.fill_rect(2, 473, 636 - bar_w, 5, 230, 180, 40);
    }
  }
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
  let title_img = match decode_spy(640, 480, TITLEBE_SPY_BYTES) {
    Ok(img) => img,
    Err(_) => return 0,
  };
  let main_menu_img = match decode_spy(640, 480, MAIN3_SPY_BYTES) {
    Ok(img) => img,
    Err(_) => return 0,
  };
  let shop_img = match decode_spy(640, 480, SHOPPIC_SPY_BYTES) {
    Ok(img) => img,
    Err(_) => return 0,
  };
  let options_img = match decode_spy(640, 480, OPTIONS5_SPY_BYTES) {
    Ok(img) => img,
    Err(_) => return 0,
  };
  let info_img = match decode_spy(640, 480, INFO1_SPY_BYTES) {
    Ok(img) => img,
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

  let prices = Prices::new();
  for p in players.iter_mut() {
    if p.is_bot {
      auto_buy_for_bot(p, &prices);
    } else {
      equip_starter_pack(p);
    }
  }

  let mut game = WebGame {
    state: AppState::Title,
    selected_menu: 0,
    selected_option: GameOption::MainMenu,
    options: opts,
    round: 1,
    total_rounds: 10,
    prices,
    players,
    shop_p1: PlayerShopState::new(),
    shop_p2: PlayerShopState::new(),
    level,
    world: None,
    sika,
    font,
    title_img,
    main_menu_img,
    shop_img,
    options_img,
    info_img,
    audio_queue: Vec::with_capacity(32),
    rumble_queue: Vec::with_capacity(16),
    round_start_tick: 0,
    max_round_ticks: 60 * 180,
    choose_hold_ticks: [0; 4],
  };

  game.render_current_state();

  unsafe {
    GAME = Some(game);
  }

  1
}

#[no_mangle]
pub extern "C" fn mb_render_title() -> u32 {
  unsafe {
    if let Some(ref mut game) = GAME {
      game.state = AppState::Title;
      game.render_current_state();
      1
    } else {
      0
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_render() -> u32 {
  unsafe {
    if let Some(ref mut game) = GAME {
      game.render_current_state();
      1
    } else {
      0
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_get_state() -> u32 {
  unsafe {
    if let Some(ref game) = GAME {
      game.state as u32
    } else {
      0
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_handle_key(key: u32) -> u32 {
  unsafe {
    if let Some(ref mut game) = GAME {
      game.handle_key(key);
      game.state as u32
    } else {
      0
    }
  }
}

#[no_mangle]
pub extern "C" fn mb_step(p1: u32, p2: u32, p3: u32, p4: u32) -> u32 {
  unsafe {
    let game = match GAME.as_mut() {
      Some(g) => g,
      None => return 0,
    };

    if game.state != AppState::Battle {
      return 1;
    }

    game.round_start_tick += 1;

    let remaining_ticks = game.max_round_ticks.saturating_sub(game.round_start_tick);
    if let Some(ref mut world) = game.world {
      if remaining_ticks <= 60 * 60 {
        world.sudden_death_warning = true;
      }
      if remaining_ticks <= 50 * 60 {
        // Every 5 seconds (300 ticks), shrink another ring
        if game.round_start_tick % 300 == 0 {
          let ring = world.advance_sudden_death_shrink();
          if ring > 0 {
            game.rumble_queue.push(RumbleEvent {
              player_idx: 0,
              intensity: 1.0,
              duration_ms: 600.0,
            });
          }
        }
      }
      if remaining_ticks == 0 {
        world.end_round_counter += 105;
      }
    }

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
        if (input_mask & 32) != 0 {
          let hold = &mut game.choose_hold_ticks[idx];
          if *hold == 0 {
            world.player_action(idx, Key::Choose);
            *hold = 1;
          } else {
            *hold += 1;
            // Delay 24 frames (~400ms), then repeat every 12 frames (~200ms)
            if *hold >= 24 && (*hold - 24) % 12 == 0 {
              world.player_action(idx, Key::Choose);
            }
          }
        } else {
          game.choose_hold_ticks[idx] = 0;
        }
        if (input_mask & 64) != 0 { world.player_action(idx, Key::Remote); }
        if (input_mask & 128) != 0 { world.player_action(idx, Key::Stop); }
      }

      world.tick();

      let effects = std::mem::take(&mut world.effects.queue);
      let updates = std::mem::take(&mut world.update.queue);
      let is_flash = world.flash;
      let is_end_round = world.is_end_of_round();

      let map_snapshots: Vec<(Cursor, MapValue)> = updates
        .iter()
        .filter_map(|u| match u {
          Update::Map(cur) => Some((*cur, world.maps.level[*cur])),
          _ => None,
        })
        .collect();

      let actors: Vec<_> = world.actors.iter().map(|a| {
        (a.pos, a.kind, a.facing, a.moving, a.animation)
      }).collect();

      (effects, updates, map_snapshots, is_flash, is_end_round, actors)
    };

    // Audio and Rumble
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
      for (cursor, val) in map_snapshots {
        game.level[cursor] = val;
        game.draw_map_tile(cursor, val);
      }

      for update in updates {
        match update {
          Update::Map(_) => {}
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

      game.round += 1;
      if game.round <= game.total_rounds {
        let next_level = LevelMap::random_map(10);
        game.level = next_level;
        for p in game.players[1..].iter_mut() {
          auto_buy_for_bot(p, &game.prices);
        }
        game.shop_p1 = PlayerShopState::new();
        game.shop_p2 = PlayerShopState::new();
        game.shop_p2.ready = true;
        game.state = AppState::Shop;
        game.render_current_state();
        return 2;
      } else {
        game.state = AppState::MainMenu;
        game.render_current_state();
        return 3;
      }
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
