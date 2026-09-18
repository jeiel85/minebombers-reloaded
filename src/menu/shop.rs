use crate::context::{Animation, ApplicationContext};
use crate::error::ApplicationError::SdlError;
use crate::glyphs::Glyph;
use crate::keys::Key;
use crate::menu::preview::generate_preview;
use crate::options::Options;
use crate::world::equipment::Equipment;
use crate::world::map::LevelMap;
use crate::world::player::PlayerComponent;
use crate::Application;
use rand::Rng;
use sdl2::keyboard::Scancode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;
use std::borrow::Cow;
use std::convert::TryFrom;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShopResult {
  ExitGame,
  Continue,
}

#[derive(Default)]
pub struct Prices {
  prices: [u32; Equipment::TOTAL],
}

struct PlayerState<'a> {
  entity: &'a mut PlayerComponent,
  /// `None` means level exit
  selection: Option<Equipment>,
  ready: bool,
  page: usize,
}

impl PlayerState<'_> {
  fn current_slot(&self) -> usize {
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

struct State<'a> {
  prices: Prices,
  remaining_rounds: u16,
  left: Option<PlayerState<'a>>,
  right: PlayerState<'a>,
}

impl Prices {
  pub fn new(free_market: bool) -> Prices {
    // free market?
    let percentage = if free_market {
      let mut rng = rand::thread_rng();
      130u32 - rng.gen_range(0..60)
    } else {
      100u32
    };

    let mut prices = Prices::default();
    for equipment in Equipment::all_equipment() {
      prices[equipment] = adjust_price(equipment.base_price(), percentage);
    }
    prices
  }
}

impl std::ops::Index<Equipment> for Prices {
  type Output = u32;

  fn index(&self, index: Equipment) -> &u32 {
    &self.prices[index as usize]
  }
}

impl std::ops::IndexMut<Equipment> for Prices {
  fn index_mut(&mut self, index: Equipment) -> &mut u32 {
    &mut self.prices[index as usize]
  }
}

impl Application<'_> {
  /// Run the shop logic
  pub fn shop(
    &self,
    ctx: &mut ApplicationContext,
    remaining_rounds: u16,
    options: &Options,
    preview_map: Option<&LevelMap>,
    shared_cash: &mut Option<u32>,
    left: Option<&mut PlayerComponent>,
    right: &mut PlayerComponent,
  ) -> Result<ShopResult, anyhow::Error> {
    let mut state = State {
      prices: Prices::new(options.free_market),
      remaining_rounds,
      left: left.map(|entity| PlayerState {
        entity,
        selection: Some(Equipment::SmallBomb),
        ready: false,
        page: 0,
      }),
      right: PlayerState {
        entity: right,
        selection: Some(Equipment::SmallBomb),
        ready: false,
        page: 0,
      },
    };

    // Auto-purchase items for CPU bots
    if let Some(left) = state.left.as_mut() {
      if left.entity.is_bot {
        auto_buy_for_bot(left.entity, &state.prices);
        left.ready = true;
      }
    }
    if state.right.entity.is_bot {
      auto_buy_for_bot(state.right.entity, &state.prices);
      state.right.ready = true;
    }

    // If both slots on this shop screen are ready (e.g. both are bots), skip interactive screen
    let all_ready = state.left.as_ref().map_or(true, |l| l.ready) && state.right.ready;
    if all_ready {
      return Ok(ShopResult::Continue);
    }

    // Render an initial shop screen
    let texture_creator = ctx.texture_creator();
    let palette = &self.shop.palette;
    ctx.with_render_context(|canvas| {
      canvas.copy(&self.shop.texture, None, None).map_err(SdlError)?;
      let remaining = state.remaining_rounds.to_string();
      self.font.render(canvas, 306, 120, palette[1], &remaining)?;

      // Background
      if let Some(left) = &state.left {
        self.render_player_stats(canvas, 0, *shared_cash, left)?;
      }
      self.render_player_stats(canvas, 420, *shared_cash, &state.right)?;

      // All shop items
      if let Some(left) = &state.left {
        self.render_all_items(canvas, 0, left, &state.prices)?;
      }
      let right = &state.right;
      self.render_all_items(canvas, 320, right, &state.prices)?;

      // Preview map
      if let Some(map) = preview_map {
        let tgt = Rect::new(288, 51, 64, 45);
        let preview = generate_preview(map, texture_creator, &self.shop.palette)?;
        canvas.copy(&preview, None, tgt).map_err(SdlError)?;
      }
      Ok(())
    })?;
    ctx.animate(Animation::FadeUp, 7)?;

    let mut result = ShopResult::Continue;
    while state.left.as_ref().map_or(false, |state| !state.ready) || !state.right.ready {
      let scan = ctx.wait_key_pressed().0;
      match scan {
        Scancode::Escape | Scancode::F10 => {
          result = ShopResult::ExitGame;
          break;
        }
        _ => {}
      }

      if let Some(left) = &mut state.left {
        self.handle_player_keys(ctx, scan, true, options.selling, shared_cash, left, &state.prices)?;
      }
      self.handle_player_keys(
        ctx,
        scan,
        false,
        options.selling,
        shared_cash,
        &mut state.right,
        &state.prices,
      )?;
    }

    ctx.animate(Animation::FadeDown, 7)?;
    Ok(result)
  }

  fn handle_player_keys(
    &self,
    ctx: &mut ApplicationContext,
    scan: Scancode,
    left: bool,
    selling: bool,
    shared_cash: &mut Option<u32>,
    state: &mut PlayerState,
    prices: &Prices,
  ) -> Result<(), anyhow::Error> {
    let last_selection = state.selection;
    let last_slot = state.current_slot();

    // Left the store already
    if state.ready {
      return Ok(());
    }

    // Toggle shop page (Tab, or Q/E for left player, PageDown/PageUp for right player)
    let toggle_page = if left {
      scan == Scancode::Tab || scan == Scancode::Q || scan == Scancode::E
    } else {
      scan == Scancode::Tab || scan == Scancode::PageDown || scan == Scancode::PageUp
    };

    if toggle_page {
      state.page = 1 - state.page;
      if state.selection.is_some() {
        if state.page == 1 {
          state.selection = Some(Equipment::BlackHole);
        } else {
          state.selection = Some(Equipment::SmallBomb);
        }
      }
      ctx.with_render_context(|canvas| {
        let offsets = if left { (0, 0) } else { (420, 320) };
        self.render_player_stats(canvas, offsets.0, *shared_cash, state)?;
        self.render_all_items(canvas, offsets.1, state, prices)?;
        Ok(())
      })?;
      ctx.present()?;
      return Ok(());
    }

    let cash = shared_cash.as_mut().unwrap_or(&mut state.entity.cash);
    if Some(scan) == state.entity.keys[Key::Bomb] {
      if let Some(selection) = state.selection {
        if *cash >= prices[selection] {
          *cash -= prices[selection];
          state.entity.inventory[selection] += 1;
          state.entity.stats.bombs_bought += 1;
        }
      } else {
        state.ready = true;
      }
    } else if Some(scan) == state.entity.keys[Key::Choose] {
      if let Some(selection) = state.selection {
        if selling && state.entity.inventory[selection] > 0 {
          // Only return 70% of the cost
          *cash += (7 * prices[selection] + 5) / 10;
          state.entity.inventory[selection] -= 1;
        }
      }
    } else if Some(scan) == state.entity.keys[Key::Right] {
      if state.page == 0 {
        let new_slot = (last_slot + 1).min(27);
        state.selection = if new_slot < 27 {
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
        state.selection = if new_slot < 3 {
          Equipment::try_from((27 + new_slot) as u8).ok()
        } else {
          None
        };
      }
    } else if Some(scan) == state.entity.keys[Key::Left] {
      if state.page == 0 {
        let new_slot = last_slot.saturating_sub(1);
        state.selection = if new_slot < 27 {
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
        state.selection = if new_slot < 3 {
          Equipment::try_from((27 + new_slot) as u8).ok()
        } else {
          None
        };
      }
    } else if Some(scan) == state.entity.keys[Key::Down] {
      if state.page == 0 {
        let new_slot = (last_slot + 4).min(27);
        state.selection = if new_slot < 27 {
          Equipment::try_from(new_slot as u8).ok()
        } else {
          None
        };
      } else {
        state.selection = None; // On Page 1, Down jumps straight to LEAVE
      }
    } else if Some(scan) == state.entity.keys[Key::Up] {
      if state.page == 0 {
        let new_slot = if last_slot >= 4 { last_slot - 4 } else { last_slot };
        state.selection = if new_slot < 27 {
          Equipment::try_from(new_slot as u8).ok()
        } else {
          None
        };
      } else {
        let new_slot = if last_slot == 27 { 2 } else { last_slot };
        state.selection = if new_slot < 3 {
          Equipment::try_from((27 + new_slot) as u8).ok()
        } else {
          None
        };
      }
    } else {
      // Nothing to re-render, skip re-rendering
      return Ok(());
    }

    let new_slot = state.current_slot();
    ctx.with_render_context(|canvas| {
      let offsets = if left { (0, 0) } else { (420, 320) };
      self.render_player_stats(canvas, offsets.0, *shared_cash, state)?;

      if last_slot != new_slot {
        self.render_shop_slot(canvas, offsets.1, last_slot, last_selection, state, prices)?;
      }
      self.render_shop_slot(canvas, offsets.1, new_slot, state.selection, state, prices)?;
      Ok(())
    })?;
    ctx.present()?;
    Ok(())
  }

  fn render_player_stats(
    &self,
    canvas: &mut WindowCanvas,
    offset_x: i32,
    shared_cash: Option<u32>,
    state: &PlayerState,
  ) -> Result<(), anyhow::Error> {
    canvas.set_draw_color(Color::BLACK);

    let palette = &self.shop.palette;
    canvas
      .fill_rect(Rect::new(35 + offset_x, 30, 7 * 8, 8))
      .map_err(SdlError)?;
    canvas
      .fill_rect(Rect::new(35 + offset_x, 58, 7 * 8, 8))
      .map_err(SdlError)?;

    let power = 1 + state.entity.initial_drilling_power();
    self
      .font
      .render(canvas, 35 + offset_x, 16, palette[1], &state.entity.stats.name)?;
    self
      .font
      .render(canvas, 35 + offset_x, 30, palette[3], &power.to_string())?;
    if let Some(cash) = shared_cash {
      let cash = cash.to_string();

      // Update cash for the both players
      canvas.fill_rect(Rect::new(35, 44, 7 * 8, 8)).map_err(SdlError)?;
      canvas.fill_rect(Rect::new(455, 44, 7 * 8, 8)).map_err(SdlError)?;
      self.font.render(canvas, 35, 44, palette[5], &cash)?;
      self.font.render(canvas, 455, 44, palette[5], &cash)?;
    } else {
      canvas
        .fill_rect(Rect::new(35 + offset_x, 44, 7 * 8, 8))
        .map_err(SdlError)?;
      self
        .font
        .render(canvas, 35 + offset_x, 44, palette[5], &state.entity.cash.to_string())?;
    }

    if let Some(item) = state.selection {
      let item_count = state.entity.inventory[item];
      self
        .font
        .render(canvas, 35 + offset_x, 58, palette[1], &item_count.to_string())?;
    }
    Ok(())
  }

  /// `None` for `selected` means that level exit is selected
  fn render_all_items(
    &self,
    canvas: &mut WindowCanvas,
    offset_x: i32,
    state: &PlayerState,
    prices: &Prices,
  ) -> Result<(), anyhow::Error> {
    if state.page == 0 {
      for slot in 0..=26 {
        let eq = Equipment::try_from(slot as u8).ok();
        self.render_shop_slot(canvas, offset_x, slot, eq, state, prices)?;
      }
    } else {
      for slot in 0..3 {
        let eq = Equipment::try_from((27 + slot) as u8).ok();
        self.render_shop_slot(canvas, offset_x, slot, eq, state, prices)?;
      }
      for slot in 3..27 {
        self.render_empty_slot(canvas, offset_x, slot)?;
      }
    }
    self.render_shop_slot(canvas, offset_x, 27, None, state, prices)?;
    self.render_page_banner(canvas, offset_x, state.page)?;
    Ok(())
  }

  fn render_empty_slot(
    &self,
    canvas: &mut WindowCanvas,
    offset_x: i32,
    slot: usize,
  ) -> Result<(), anyhow::Error> {
    let col = (slot % 4) as i32;
    let row = (slot / 4) as i32;
    let pos_x = col * 64 + 32 + offset_x;
    let pos_y = row * 48 + 96;
    self.glyphs.render(canvas, pos_x, pos_y, Glyph::ShopSlot(false))?;
    Ok(())
  }

  fn render_page_banner(
    &self,
    canvas: &mut WindowCanvas,
    offset_x: i32,
    page: usize,
  ) -> Result<(), anyhow::Error> {
    let palette = &self.shop.palette;
    canvas.set_draw_color(Color::BLACK);
    canvas
      .fill_rect(Rect::new(80 + offset_x, 442, 160, 14))
      .map_err(SdlError)?;
    let banner_text = if page == 0 {
      "PAGE 1/2 [TAB]"
    } else {
      "PAGE 2/2 [TAB]"
    };
    self.font.render(canvas, 96 + offset_x, 444, palette[1], banner_text)?;
    Ok(())
  }

  /// `slot_index` is 0..=27 on the current page
  fn render_shop_slot(
    &self,
    canvas: &mut WindowCanvas,
    offset_x: i32,
    slot_index: usize,
    slot: Option<Equipment>,
    state: &PlayerState,
    prices: &Prices,
  ) -> Result<(), anyhow::Error> {
    let palette = &self.shop.palette;

    let col = (slot_index % 4) as i32;
    let row = (slot_index / 4) as i32;

    let pos_x = col * 64 + 32 + offset_x;
    let pos_y = row * 48 + 96;
    let is_selected = state.selection == slot;
    self
      .glyphs
      .render(canvas, pos_x, pos_y, Glyph::ShopSlot(is_selected))?;

    // Render item count
    let item_count = slot.map(|item| state.entity.inventory[item] as i32).unwrap_or(0);
    if item_count != 0 {
      let pos_x = col * 64 + 88 + offset_x;
      let pos_y = row * 48 + 99;
      let delta = 40 - ((item_count * 2).min(40));
      for (idx, color) in [14, 13, 12, 11, 7].iter().copied().enumerate() {
        let idx = idx as i32;
        canvas.set_draw_color(palette[color]);
        canvas
          .draw_line((pos_x + idx, pos_y + delta), (pos_x + idx, pos_y + 41))
          .map_err(SdlError)?;
      }
    }

    // Render item glyph
    let pos_x = col * 64 + 49 + offset_x;
    let pos_y = row * 48 + 99;
    let glyph = slot.map(Glyph::Selection).unwrap_or(Glyph::Ready);
    self.glyphs.render(canvas, pos_x, pos_y, glyph)?;

    // Render item price
    let pos_x = col * 64 + 44 + offset_x;
    let pos_y = row * 48 + 132;

    let text = slot
      .map(|slot| Cow::Owned(format!("{}$", prices[slot])))
      .unwrap_or_else(|| Cow::Borrowed("LEAVE"));
    self.font.render(canvas, pos_x, pos_y, palette[5], &text)?;
    Ok(())
  }
}

fn adjust_price(price: u32, percentage: u32) -> u32 {
  ((price - 1) * percentage + 50) / 100 + 1
}

fn auto_buy_for_bot(player: &mut PlayerComponent, prices: &Prices) {
  use crate::world::bot::BotDifficulty;
  let max_armor = match player.bot_difficulty {
    BotDifficulty::Hard => 3,
    BotDifficulty::Medium => 2,
    BotDifficulty::Easy => 1,
  };
  // Buy armor
  while player.cash >= prices[Equipment::Armor] && player.inventory[Equipment::Armor] < max_armor {
    player.cash -= prices[Equipment::Armor];
    player.inventory[Equipment::Armor] += 1;
  }

  // Buy pickaxe or drill
  if player.inventory[Equipment::Drill] == 0 && player.cash >= prices[Equipment::Drill] {
    player.cash -= prices[Equipment::Drill];
    player.inventory[Equipment::Drill] += 1;
  } else if player.inventory[Equipment::LargePickaxe] == 0 && player.cash >= prices[Equipment::LargePickaxe] {
    player.cash -= prices[Equipment::LargePickaxe];
    player.inventory[Equipment::LargePickaxe] += 1;
  }

  // Buy bombs based on difficulty
  let max_bombs = match player.bot_difficulty {
    BotDifficulty::Hard => 15,
    BotDifficulty::Medium => 10,
    BotDifficulty::Easy => 6,
  };
  while player.cash >= prices[Equipment::SmallBomb] && player.inventory[Equipment::SmallBomb] < max_bombs {
    player.cash -= prices[Equipment::SmallBomb];
    player.inventory[Equipment::SmallBomb] += 1;
  }

  // Dynamite & Grenades (not for Easy bots)
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

  // Hard bot buys Big Bombs & Remote Bombs if affordable
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

