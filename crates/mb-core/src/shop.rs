//! Shop rules shared by the native game and the web edition: equipment prices and what a CPU
//! player buys between rounds.
use crate::world::bot::BotDifficulty;
use crate::world::equipment::Equipment;
use crate::world::player::PlayerComponent;
use rand::Rng;

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

/// Equip a CPU player with `prices`, spending its cash the way its difficulty dictates. Also used by
/// `world::bot`'s difficulty-balance measurement, so bots there are equipped the way a real game does.
pub fn auto_buy_for_bot(player: &mut PlayerComponent, prices: &Prices) {
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
    let max_dynamite = if player.bot_difficulty == BotDifficulty::Hard {
      8
    } else {
      5
    };
    while player.cash >= prices[Equipment::Dynamite] && player.inventory[Equipment::Dynamite] < max_dynamite {
      player.cash -= prices[Equipment::Dynamite];
      player.inventory[Equipment::Dynamite] += 1;
    }
    let max_grenades = if player.bot_difficulty == BotDifficulty::Hard {
      6
    } else {
      4
    };
    while player.cash >= prices[Equipment::Grenade] && player.inventory[Equipment::Grenade] < max_grenades {
      player.cash -= prices[Equipment::Grenade];
      player.inventory[Equipment::Grenade] += 1;
    }
  }

  // The cheap specials the AI knows how to use (see `BotParams::special_weapons`). They are listed
  // after the staples on purpose: an extinguisher is worth having, but not instead of bombs.
  if player.bot_difficulty != BotDifficulty::Easy {
    // Puts out a fuse up to six tiles away - what the bot reaches for when a blast covers it and
    // there is nowhere to run.
    if player.inventory[Equipment::Extinguisher] == 0 && player.cash >= prices[Equipment::Extinguisher] {
      player.cash -= prices[Equipment::Extinguisher];
      player.inventory[Equipment::Extinguisher] += 1;
    }
  }

  // Hard bot buys Big Bombs & Remote Bombs if affordable
  if player.bot_difficulty == BotDifficulty::Hard {
    // Radio bombs are armed rather than fused, so a Hard bot can leave one in a chaser's path and
    // set it off from a safe distance. At 15 each they are the cheapest thing in the shop.
    while player.cash >= prices[Equipment::SmallRadio] && player.inventory[Equipment::SmallRadio] < 3 {
      player.cash -= prices[Equipment::SmallRadio];
      player.inventory[Equipment::SmallRadio] += 1;
    }
    // A blast down the whole row and column it lands on, for 35.
    while player.cash >= prices[Equipment::SmallCrucifix] && player.inventory[Equipment::SmallCrucifix] < 2 {
      player.cash -= prices[Equipment::SmallCrucifix];
      player.inventory[Equipment::SmallCrucifix] += 1;
    }

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
