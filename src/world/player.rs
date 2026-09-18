use crate::keys::KeyBindings;
use crate::options::Options;
use crate::roster::RosterInfo;
use crate::world::equipment::Equipment;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GlyphCheat {
  /// Render player as a slime
  Slime,
  /// Don't render player at all!
  Invisible,
}

/// Component corresponding to the active player
#[derive(Default)]
pub struct PlayerComponent {
  /// Player name and statistics
  pub stats: RosterInfo,
  /// Index of the player in the players roster (PLAYERS.DAT).
  pub roster_index: u8,
  /// Player keybindings
  pub keys: KeyBindings,
  /// Cash that will not be lost on death. All accumulated cash is moved into this bucket if
  /// player survives level.
  pub cash: u32,
  /// Player inventory
  pub inventory: Inventory,
  /// Currently selected item
  pub selection: Equipment,
  /// For single player mode, tracks amount of lives player has. Not used in multi-player mode.
  pub lives: u16,
  /// For multi-player mode, amount of won rounds (separate from stats, which tracks rounds won
  /// across all games).
  pub rounds_win: u32,
  /// Whether this player is controlled by AI bot
  pub is_bot: bool,
  /// AI Bot difficulty level (Easy, Medium, Hard)
  pub bot_difficulty: crate::world::bot::BotDifficulty,
}

impl PlayerComponent {
  pub fn new(
    name: String,
    keys: KeyBindings,
    options: &Options,
    is_bot: bool,
    bot_difficulty: crate::world::bot::BotDifficulty,
  ) -> Self {
    let mut player = PlayerComponent {
      stats: RosterInfo {
        name,
        ..Default::default()
      },
      keys,
      cash: u32::from(options.cash),
      is_bot,
      bot_difficulty,
      ..Default::default()
    };

    // Apply some of the cheat codes. Note that we also allow cheats in a single player game (contrary to the original game).
    match player.stats.name.as_str() {
      "Lottery" => {
        player.cash = 50000;
      }
      "Skitso" => {
        for equipment in Equipment::all_equipment() {
          match equipment {
            Equipment::Armor => {}
            Equipment::SmallPickaxe | Equipment::LargePickaxe | Equipment::Drill => {
              player.inventory[equipment] = 1;
            }
            _ => {
              player.inventory[equipment] = 50;
            }
          }
        }
      }
      "Pyroman" => {
        player.inventory[Equipment::Flamethrower] = 1000;
      }
      _ => {}
    }
    player
  }

  pub fn initial_drilling_power(&self) -> u16 {
    self.inventory[Equipment::SmallPickaxe] * 3
      + 8 * self.inventory[Equipment::LargePickaxe]
      + 20 * self.inventory[Equipment::Drill]
  }

  pub fn initial_health(&self) -> u16 {
    // Cheat code -- almost invulnerable
    if self.stats.name == "Rambo" {
      32000
    } else {
      100 + 100 * self.inventory[Equipment::Armor]
    }
  }

  /// Return an override for glyph that should be rendered for this player
  pub fn glyph_cheat(&self) -> Option<GlyphCheat> {
    match self.stats.name.as_str() {
      "Invis" => Some(GlyphCheat::Invisible),
      "Mutation" => Some(GlyphCheat::Slime),
      _ => None,
    }
  }
}

#[derive(Default, Clone, Copy)]
pub struct Inventory {
  inventory: [u16; Equipment::TOTAL],
}

impl std::ops::Index<Equipment> for Inventory {
  type Output = u16;

  fn index(&self, index: Equipment) -> &u16 {
    &self.inventory[index as usize]
  }
}

impl std::ops::IndexMut<Equipment> for Inventory {
  fn index_mut(&mut self, index: Equipment) -> &mut u16 {
    &mut self.inventory[index as usize]
  }
}
