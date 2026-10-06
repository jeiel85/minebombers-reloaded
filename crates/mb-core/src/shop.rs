//! Shop rules shared by the native game and the web edition: equipment prices, moving the cursor,
//! buying and selling, and what a CPU player buys between rounds.
use crate::world::equipment::Equipment;
use crate::world::player::PlayerComponent;
use crate::world::position::Direction;
use rand::Rng;
use std::convert::TryFrom;

#[derive(Clone, Copy)]
pub struct Prices {
  prices: [u32; Equipment::TOTAL],
}

impl Default for Prices {
  /// Base prices, without free market adjustment
  fn default() -> Self {
    Prices::new(false)
  }
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

    let mut prices = Prices {
      prices: [0; Equipment::TOTAL],
    };
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

fn adjust_price(price: u32, percentage: u32) -> u32 {
  ((price - 1) * percentage + 50) / 100 + 1
}

/// Slot of the LEAVE button, after the 27 items of the first page (4 per row).
pub const LEAVE_SLOT: usize = 27;
/// Items on the first page; the rest (`BlackHole` and on) are on the second page.
const FIRST_PAGE_ITEMS: usize = 27;
const ROW: usize = 4;

/// Where one player's cursor is in the shop: an item, or LEAVE (`selection` of `None`), on page 0 or 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShopCursor {
  pub selection: Option<Equipment>,
  pub page: usize,
}

impl Default for ShopCursor {
  fn default() -> Self {
    ShopCursor {
      selection: Some(Equipment::SmallBomb),
      page: 0,
    }
  }
}

impl ShopCursor {
  /// Output: the slot on the current page, `LEAVE_SLOT` for LEAVE. Page 1 numbers its items from 0.
  pub fn slot(&self) -> usize {
    match (self.page, self.selection) {
      (0, Some(eq)) => (eq as usize).min(FIRST_PAGE_ITEMS - 1),
      (1, Some(eq)) => (eq as usize).checked_sub(FIRST_PAGE_ITEMS).unwrap_or(0),
      _ => LEAVE_SLOT,
    }
  }

  /// Switch pages. A cursor on an item moves to the first item of the other page; LEAVE stays LEAVE.
  pub fn toggle_page(&mut self) {
    self.page = 1 - self.page;
    if self.selection.is_some() {
      self.selection = Some(if self.page == 1 {
        Equipment::BlackHole
      } else {
        Equipment::SmallBomb
      });
    }
  }

  /// Move the cursor one step. Page 0 is a grid of 4 per row with LEAVE after the last item; page 1 is
  /// a single row, and Down on it goes straight to LEAVE.
  pub fn step(&mut self, direction: Direction) {
    let slot = self.slot();
    let new_slot = if self.page == 0 {
      match direction {
        Direction::Right => (slot + 1).min(LEAVE_SLOT),
        Direction::Left => slot.saturating_sub(1),
        Direction::Down => (slot + ROW).min(LEAVE_SLOT),
        Direction::Up if slot >= ROW => slot - ROW,
        Direction::Up => slot,
      }
    } else {
      match (direction, slot) {
        (Direction::Right, 0) => 1,
        (Direction::Right, 1) => 2,
        (Direction::Right, _) => LEAVE_SLOT,
        (Direction::Left, LEAVE_SLOT) => 2,
        (Direction::Left, 2) => 1,
        (Direction::Left, _) => 0,
        (Direction::Down, _) => LEAVE_SLOT,
        (Direction::Up, LEAVE_SLOT) => 2,
        (Direction::Up, slot) => slot,
      }
    };
    let first = if self.page == 0 { 0 } else { FIRST_PAGE_ITEMS };
    self.selection = if new_slot == LEAVE_SLOT {
      None
    } else {
      u8::try_from(first + new_slot)
        .ok()
        .and_then(|v| Equipment::try_from(v).ok())
    };
  }
}

/// Input: the buyer, the shared cash in a campaign for several players (`None`: the player's own), the
/// item and the prices.
/// Output: whether the item was bought (the buyer could afford it).
pub fn buy(player: &mut PlayerComponent, shared_cash: Option<&mut u32>, item: Equipment, prices: &Prices) -> bool {
  let cash = match shared_cash {
    Some(cash) => cash,
    None => &mut player.cash,
  };
  if *cash < prices[item] {
    return false;
  }
  *cash -= prices[item];
  player.inventory[item] += 1;
  player.stats.bombs_bought += 1;
  true
}

/// Input: as `buy`, plus whether the game options allow selling.
/// Output: whether one item was sold, for 70% of its price (rounded).
pub fn sell(
  player: &mut PlayerComponent,
  shared_cash: Option<&mut u32>,
  item: Equipment,
  prices: &Prices,
  selling_allowed: bool,
) -> bool {
  if !selling_allowed || player.inventory[item] == 0 {
    return false;
  }
  let cash = match shared_cash {
    Some(cash) => cash,
    None => &mut player.cash,
  };
  *cash += (7 * prices[item] + 5) / 10;
  player.inventory[item] -= 1;
  true
}

/// What every CPU player buys between rounds, whatever its difficulty: item, how many it wants to
/// hold. Difficulty is only how well the bot plays (`world::bot::BotParams`), not what it carries -
/// a player choosing an easier opponent wants a clumsier one, not one with fewer bombs.
const BOT_LOADOUT: [(Equipment, u16); 5] = [
  (Equipment::Armor, 2),
  (Equipment::SmallBomb, 10),
  (Equipment::Dynamite, 5),
  (Equipment::Grenade, 4),
  // Puts out a fuse up to six tiles away - what a bot reaches for when a blast covers it and there
  // is nowhere to run.
  (Equipment::Extinguisher, 1),
];

/// Equip a CPU player with `prices` from `BOT_LOADOUT`, the same for every difficulty. Also used by
/// `world::bot`'s difficulty-balance measurement, so bots there are equipped the way a real game does.
pub fn auto_buy_for_bot(player: &mut PlayerComponent, prices: &Prices) {
  // Digging power first: a drill, or a large pickaxe when the drill is out of reach.
  if player.inventory[Equipment::Drill] == 0 && player.cash >= prices[Equipment::Drill] {
    player.cash -= prices[Equipment::Drill];
    player.inventory[Equipment::Drill] += 1;
  } else if player.inventory[Equipment::LargePickaxe] == 0 && player.cash >= prices[Equipment::LargePickaxe] {
    player.cash -= prices[Equipment::LargePickaxe];
    player.inventory[Equipment::LargePickaxe] += 1;
  }

  for (item, wanted) in BOT_LOADOUT {
    while player.cash >= prices[item] && player.inventory[item] < wanted {
      player.cash -= prices[item];
      player.inventory[item] += 1;
    }
  }

  player.selection = Equipment::SmallBomb;
}

#[cfg(test)]
mod tests {
  use super::*;

  fn cursor(page: usize, selection: Option<Equipment>) -> ShopCursor {
    ShopCursor { selection, page }
  }

  fn item(index: u8) -> Option<Equipment> {
    Some(Equipment::try_from(index).unwrap())
  }

  fn stepped(mut c: ShopCursor, direction: Direction) -> ShopCursor {
    c.step(direction);
    c
  }

  #[test]
  fn first_page_is_a_grid_of_four_with_leave_after_the_last_item() {
    assert_eq!(stepped(cursor(0, item(0)), Direction::Left), cursor(0, item(0)));
    assert_eq!(stepped(cursor(0, item(0)), Direction::Right), cursor(0, item(1)));
    assert_eq!(stepped(cursor(0, item(26)), Direction::Right), cursor(0, None));
    assert_eq!(stepped(cursor(0, item(5)), Direction::Down), cursor(0, item(9)));
    assert_eq!(stepped(cursor(0, item(24)), Direction::Down), cursor(0, None));
    assert_eq!(stepped(cursor(0, item(2)), Direction::Up), cursor(0, item(2)));
    assert_eq!(stepped(cursor(0, None), Direction::Up), cursor(0, item(23)));
    assert_eq!(stepped(cursor(0, None), Direction::Left), cursor(0, item(26)));
    assert_eq!(stepped(cursor(0, None), Direction::Right), cursor(0, None));
  }

  #[test]
  fn second_page_is_one_row_and_down_goes_to_leave() {
    assert_eq!(stepped(cursor(1, item(27)), Direction::Right), cursor(1, item(28)));
    assert_eq!(stepped(cursor(1, item(29)), Direction::Right), cursor(1, None));
    assert_eq!(stepped(cursor(1, None), Direction::Left), cursor(1, item(29)));
    assert_eq!(stepped(cursor(1, item(27)), Direction::Left), cursor(1, item(27)));
    assert_eq!(stepped(cursor(1, item(28)), Direction::Down), cursor(1, None));
    assert_eq!(stepped(cursor(1, None), Direction::Up), cursor(1, item(29)));
    assert_eq!(stepped(cursor(1, item(28)), Direction::Up), cursor(1, item(28)));
  }

  #[test]
  fn toggling_the_page_moves_an_item_cursor_to_the_first_item_and_keeps_leave() {
    let mut c = cursor(0, item(13));
    c.toggle_page();
    assert_eq!(c, cursor(1, Some(Equipment::BlackHole)));
    c.toggle_page();
    assert_eq!(c, cursor(0, Some(Equipment::SmallBomb)));
    let mut leave = cursor(0, None);
    leave.toggle_page();
    assert_eq!(leave, cursor(1, None));
    assert_eq!(leave.slot(), LEAVE_SLOT);
  }

  fn player(cash: u32) -> PlayerComponent {
    let mut player = PlayerComponent::default();
    player.cash = cash;
    player
  }

  #[test]
  fn buying_needs_the_full_price_and_counts_the_purchase() {
    let prices = Prices::default();
    let price = prices[Equipment::SmallBomb];
    let mut p = player(price);
    assert!(buy(&mut p, None, Equipment::SmallBomb, &prices));
    assert_eq!(
      (p.cash, p.inventory[Equipment::SmallBomb], p.stats.bombs_bought),
      (0, 1, 1)
    );
    assert!(!buy(&mut p, None, Equipment::SmallBomb, &prices));
    assert_eq!(p.inventory[Equipment::SmallBomb], 1);
  }

  #[test]
  fn shared_cash_pays_instead_of_the_players_own() {
    let prices = Prices::default();
    let price = prices[Equipment::Dynamite];
    let mut p = player(0);
    let mut shared = price;
    assert!(buy(&mut p, Some(&mut shared), Equipment::Dynamite, &prices));
    assert_eq!((shared, p.cash, p.inventory[Equipment::Dynamite]), (0, 0, 1));
    assert!(sell(&mut p, Some(&mut shared), Equipment::Dynamite, &prices, true));
    assert_eq!((shared, p.cash), ((7 * price + 5) / 10, 0));
  }

  #[test]
  fn selling_returns_70_percent_and_only_when_the_options_allow_it() {
    let prices = Prices::default();
    let price = prices[Equipment::BigBomb];
    let mut p = player(0);
    p.inventory[Equipment::BigBomb] = 1;
    assert!(!sell(&mut p, None, Equipment::BigBomb, &prices, false));
    assert_eq!((p.cash, p.inventory[Equipment::BigBomb]), (0, 1));
    assert!(sell(&mut p, None, Equipment::BigBomb, &prices, true));
    assert_eq!((p.cash, p.inventory[Equipment::BigBomb]), ((7 * price + 5) / 10, 0));
    assert!(!sell(&mut p, None, Equipment::BigBomb, &prices, true));
  }
}
