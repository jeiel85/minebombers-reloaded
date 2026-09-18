use num_enum::TryFromPrimitive;
use std::convert::TryInto;

/// Types of equipment that could be stored in an inventory and bought in the shop. Note that
/// ordering is the same as shop ordering (left to right, top to bottom).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, Default)]
pub enum Equipment {
  #[default]
  SmallBomb,
  BigBomb,
  Dynamite,
  AtomicBomb,
  SmallRadio,
  LargeRadio,
  Grenade,
  Mine,
  Flamethrower,
  Napalm,
  Barrel,
  SmallCrucifix,
  LargeCrucifix,
  Plastic,
  ExplosivePlastic,
  Digger,
  MetalWall,
  SmallPickaxe,
  LargePickaxe,
  Drill,
  Teleport,
  Clone,
  Biomass,
  Extinguisher,
  Armor,
  JumpingBomb,
  SuperDrill,
  BlackHole,
  FreezeBomb,
  DrillDrone,
}

impl Equipment {
  pub const TOTAL: usize = 30;

  const PRICES: [u32; Equipment::TOTAL] = [
    1, 3, 10, 650, 15, 65, 300, 25, 500, 80, 90, 35, 145, 15, 80, 120, 50, 400, 1100, 1600, 70, 400, 50, 80, 800, 95,
    575, 1200, 350, 450,
  ];

  pub fn all_equipment() -> impl Iterator<Item = Equipment> {
    (0..Self::TOTAL as u8).map(|v| v.try_into().unwrap())
  }

  pub fn base_price(self) -> u32 {
    Self::PRICES[self as usize]
  }

  /// Create an iterator that loops over all inventory items starting from the given one
  pub fn selection_iter(self) -> impl Iterator<Item = Equipment> {
    SelectionIter {
      start: self,
      current: self,
    }
  }
}

struct SelectionIter {
  start: Equipment,
  current: Equipment,
}

impl Iterator for SelectionIter {
  type Item = Equipment;

  fn next(&mut self) -> Option<Self::Item> {
    let next: Equipment = (((self.current as u8) + 1) % (Equipment::TOTAL as u8))
      .try_into()
      .unwrap();
    if next == self.start {
      None
    } else {
      self.current = next;
      Some(next)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::convert::TryFrom;

  #[test]
  fn test_total_equipment_count() {
    assert_eq!(Equipment::TOTAL, 30);
    assert_eq!(Equipment::all_equipment().count(), 30);
  }

  #[test]
  fn test_new_special_weapons() {
    assert_eq!(Equipment::BlackHole as u8, 27);
    assert_eq!(Equipment::FreezeBomb as u8, 28);
    assert_eq!(Equipment::DrillDrone as u8, 29);

    assert_eq!(Equipment::BlackHole.base_price(), 1200);
    assert_eq!(Equipment::FreezeBomb.base_price(), 350);
    assert_eq!(Equipment::DrillDrone.base_price(), 450);

    assert_eq!(Equipment::try_from(27), Ok(Equipment::BlackHole));
    assert_eq!(Equipment::try_from(28), Ok(Equipment::FreezeBomb));
    assert_eq!(Equipment::try_from(29), Ok(Equipment::DrillDrone));
    assert!(Equipment::try_from(30).is_err());
  }

  #[test]
  fn test_equipment_selection_iter() {
    let iter_items: Vec<_> = Equipment::BlackHole.selection_iter().collect();
    assert_eq!(iter_items.len(), 29);
    assert!(iter_items.contains(&Equipment::FreezeBomb));
    assert!(iter_items.contains(&Equipment::DrillDrone));
    assert!(iter_items.contains(&Equipment::SmallBomb));
  }
}

