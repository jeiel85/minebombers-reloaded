use crate::context::{Animation, ApplicationContext};
use crate::error::ApplicationError::SdlError;
use crate::glyphs::Glyph;
use crate::keys::{Key, ScancodeBindings};
use crate::menu::preview::generate_preview;
use crate::options::Options;
use crate::world::equipment::Equipment;
use crate::world::map::LevelMap;
use crate::world::player::PlayerComponent;
use crate::world::position::Direction;
use crate::Application;
use mb_core::keys::KeyBindings as PlayerKeys;
use mb_core::shop::{auto_buy_for_bot, buy, sell, Prices, ShopCursor};
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

/// What one key press does for one player in the shop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShopCommand {
  /// Buy the item under the cursor, or leave the shop on LEAVE.
  Buy,
  Sell,
  Move(Direction),
  TogglePage,
}

/// Input: the key pressed, the keys of the player it is decided for, the keys of the other player in the
/// same shop (if there is one), and whether this player is on the left.
/// Output: what the key does for this player, or `None`.
/// Why this order: the player's own keys come first, and the extra page keys (Tab, Q, E on the left;
/// Tab, PageDown, PageUp on the right) count only when the other player has not bound them. The
/// default keys of player 2 are Q, E and Tab for Stop, Buy and Sell, so checking the page keys first
/// left player 2 unable to buy or leave, and turned player 1's page on every sale (#46). The player's
/// own Stop key, unused in the shop otherwise, always turns the page.
fn shop_command(scan: Scancode, own: &PlayerKeys, other: Option<&PlayerKeys>, left: bool) -> Option<ShopCommand> {
  let own_key = |key| own.scancode(key) == Some(scan);
  if own_key(Key::Bomb) {
    return Some(ShopCommand::Buy);
  }
  if own_key(Key::Choose) {
    return Some(ShopCommand::Sell);
  }
  for (key, direction) in [
    (Key::Right, Direction::Right),
    (Key::Left, Direction::Left),
    (Key::Down, Direction::Down),
    (Key::Up, Direction::Up),
  ] {
    if own_key(key) {
      return Some(ShopCommand::Move(direction));
    }
  }
  if own_key(Key::Stop) {
    return Some(ShopCommand::TogglePage);
  }
  let page_keys = if left {
    [Scancode::Tab, Scancode::Q, Scancode::E]
  } else {
    [Scancode::Tab, Scancode::PageDown, Scancode::PageUp]
  };
  let taken_by_other = other.map_or(false, |other| {
    Key::all_keys().any(|key| other.scancode(key) == Some(scan))
  });
  if page_keys.contains(&scan) && !taken_by_other {
    return Some(ShopCommand::TogglePage);
  }
  None
}

/// Input: an SDL key name such as "Keypad 5" or "Right Shift".
/// Output: the name in at most 7 characters for the page banner (which has room for 18), shortened so
/// the part that tells keys apart survives: "KP 5", "R SHIFT".
fn banner_key_name(name: &str) -> String {
  const MAX_CHARS: usize = 7;
  let name = name
    .to_uppercase()
    .replace("KEYPAD ", "KP ")
    .replace("LEFT ", "L ")
    .replace("RIGHT ", "R ");
  name.chars().take(MAX_CHARS).collect()
}

struct PlayerState<'a> {
  entity: &'a mut PlayerComponent,
  cursor: ShopCursor,
  ready: bool,
}

struct State<'a> {
  prices: Prices,
  remaining_rounds: u16,
  left: Option<PlayerState<'a>>,
  right: PlayerState<'a>,
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
        cursor: ShopCursor::default(),
        ready: false,
      }),
      right: PlayerState {
        entity: right,
        cursor: ShopCursor::default(),
        ready: false,
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

      let right_keys = state.right.entity.keys;
      let left_keys = state.left.as_ref().map(|left| left.entity.keys);
      if let Some(left) = &mut state.left {
        let command = shop_command(scan, &left.entity.keys, Some(&right_keys), true);
        self.handle_player_command(ctx, command, true, options.selling, shared_cash, left, &state.prices)?;
      }
      let command = shop_command(scan, &right_keys, left_keys.as_ref(), false);
      self.handle_player_command(
        ctx,
        command,
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

  fn handle_player_command(
    &self,
    ctx: &mut ApplicationContext,
    command: Option<ShopCommand>,
    left: bool,
    selling: bool,
    shared_cash: &mut Option<u32>,
    state: &mut PlayerState,
    prices: &Prices,
  ) -> Result<(), anyhow::Error> {
    let last_selection = state.cursor.selection;
    let last_slot = state.cursor.slot();

    // Left the store already
    if state.ready {
      return Ok(());
    }

    match command {
      None => {
        // Nothing to re-render, skip re-rendering
        return Ok(());
      }
      Some(ShopCommand::TogglePage) => {
        state.cursor.toggle_page();
        ctx.with_render_context(|canvas| {
          let offsets = if left { (0, 0) } else { (420, 320) };
          self.render_player_stats(canvas, offsets.0, *shared_cash, state)?;
          self.render_all_items(canvas, offsets.1, state, prices)?;
          Ok(())
        })?;
        ctx.present()?;
        return Ok(());
      }
      Some(ShopCommand::Buy) => match state.cursor.selection {
        Some(selection) => {
          buy(state.entity, shared_cash.as_mut(), selection, prices);
        }
        None => state.ready = true,
      },
      Some(ShopCommand::Sell) => {
        if let Some(selection) = state.cursor.selection {
          sell(state.entity, shared_cash.as_mut(), selection, prices, selling);
        }
      }
      Some(ShopCommand::Move(direction)) => state.cursor.step(direction),
    }

    let new_slot = state.cursor.slot();
    ctx.with_render_context(|canvas| {
      let offsets = if left { (0, 0) } else { (420, 320) };
      self.render_player_stats(canvas, offsets.0, *shared_cash, state)?;

      if last_slot != new_slot {
        self.render_shop_slot(canvas, offsets.1, last_slot, last_selection, state, prices)?;
      }
      self.render_shop_slot(canvas, offsets.1, new_slot, state.cursor.selection, state, prices)?;
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

    if let Some(item) = state.cursor.selection {
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
    if state.cursor.page == 0 {
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
    self.render_page_banner(canvas, offset_x, state)?;
    Ok(())
  }

  fn render_empty_slot(&self, canvas: &mut WindowCanvas, offset_x: i32, slot: usize) -> Result<(), anyhow::Error> {
    let col = (slot % 4) as i32;
    let row = (slot / 4) as i32;
    let pos_x = col * 64 + 32 + offset_x;
    let pos_y = row * 48 + 96;
    self.glyphs.render(canvas, pos_x, pos_y, Glyph::ShopSlot(false))?;
    Ok(())
  }

  /// "PAGE 1/2 [key]", naming the player's own Stop key, which always turns the page (`shop_command`).
  fn render_page_banner(
    &self,
    canvas: &mut WindowCanvas,
    offset_x: i32,
    state: &PlayerState,
  ) -> Result<(), anyhow::Error> {
    let palette = &self.shop.palette;
    canvas.set_draw_color(Color::BLACK);
    canvas
      .fill_rect(Rect::new(80 + offset_x, 442, 160, 14))
      .map_err(SdlError)?;
    let key_name = match state.entity.keys.scancode(Key::Stop) {
      Some(scancode) => banner_key_name(&scancode.name()),
      None => "TAB".to_string(),
    };
    let banner_text = format!("PAGE {}/2 [{}]", state.cursor.page + 1, key_name);
    self.font.render(canvas, 96 + offset_x, 444, palette[1], &banner_text)?;
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
    let is_selected = state.cursor.selection == slot;
    self.glyphs.render(canvas, pos_x, pos_y, Glyph::ShopSlot(is_selected))?;

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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::{default_player1_keys, default_player2_keys, default_player3_keys, default_player4_keys};

  fn keys(config: crate::config::PlayerKeysConfig) -> PlayerKeys {
    config.to_key_bindings().to_player_keys()
  }

  #[test]
  fn player_2_on_the_left_can_buy_sell_and_turn_the_page_with_the_default_keys() {
    let (p1, p2) = (keys(default_player1_keys()), keys(default_player2_keys()));
    let command = |scan| shop_command(scan, &p2, Some(&p1), true);
    assert_eq!(command(Scancode::E), Some(ShopCommand::Buy));
    assert_eq!(command(Scancode::Tab), Some(ShopCommand::Sell));
    assert_eq!(command(Scancode::Q), Some(ShopCommand::TogglePage));
    assert_eq!(command(Scancode::A), Some(ShopCommand::Move(Direction::Left)));
    assert_eq!(command(Scancode::S), Some(ShopCommand::Move(Direction::Down)));
    // Player 1's keys do nothing for player 2.
    assert_eq!(command(Scancode::Return), None);
    assert_eq!(command(Scancode::Space), None);
  }

  #[test]
  fn player_1_on_the_right_ignores_the_keys_of_player_2() {
    let (p1, p2) = (keys(default_player1_keys()), keys(default_player2_keys()));
    let command = |scan| shop_command(scan, &p1, Some(&p2), false);
    assert_eq!(command(Scancode::Return), Some(ShopCommand::Buy));
    assert_eq!(command(Scancode::RShift), Some(ShopCommand::Sell));
    assert_eq!(command(Scancode::Up), Some(ShopCommand::Move(Direction::Up)));
    assert_eq!(command(Scancode::Space), Some(ShopCommand::TogglePage));
    assert_eq!(command(Scancode::PageDown), Some(ShopCommand::TogglePage));
    // Tab is player 2's Sell key, so it must not turn player 1's page.
    assert_eq!(command(Scancode::Tab), None);
    assert_eq!(command(Scancode::E), None);
  }

  #[test]
  fn the_banner_keeps_the_part_of_a_key_name_that_tells_keys_apart() {
    assert_eq!(banner_key_name(&Scancode::Kp5.name()), "KP 5");
    assert_eq!(banner_key_name(&Scancode::RShift.name()), "R SHIFT");
    assert_eq!(banner_key_name(&Scancode::Space.name()), "SPACE");
    assert_eq!(banner_key_name(&Scancode::Q.name()), "Q");
    assert_eq!(banner_key_name("Right"), "RIGHT");
  }

  #[test]
  fn a_player_alone_in_the_shop_keeps_tab_for_the_page() {
    let p1 = keys(default_player1_keys());
    assert_eq!(
      shop_command(Scancode::Tab, &p1, None, false),
      Some(ShopCommand::TogglePage)
    );
  }

  #[test]
  fn players_3_and_4_share_a_shop_without_conflicts() {
    let (p3, p4) = (keys(default_player3_keys()), keys(default_player4_keys()));
    assert_eq!(shop_command(Scancode::O, &p3, Some(&p4), false), Some(ShopCommand::Buy));
    assert_eq!(
      shop_command(Scancode::Kp0, &p4, Some(&p3), true),
      Some(ShopCommand::Buy)
    );
    assert_eq!(
      shop_command(Scancode::Tab, &p4, Some(&p3), true),
      Some(ShopCommand::TogglePage)
    );
    assert_eq!(shop_command(Scancode::Kp0, &p3, Some(&p4), false), None);
  }
}
