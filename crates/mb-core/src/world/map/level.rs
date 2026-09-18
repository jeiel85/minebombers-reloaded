use super::{Map, MAP_COLS, MAP_ROWS};
use crate::world::actor::ActorKind;
use crate::world::position::{Cursor, Direction};
use num_enum::TryFromPrimitive;
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Invalid map format")]
pub struct InvalidMap;

#[derive(Debug, Error)]
#[error("Single player map '{path}' cannot be loaded")]
pub struct CannotLoadSinglePlayer {
  path: PathBuf,
  #[source]
  source: anyhow::Error,
}

pub type LevelMap = Map<MapValue>;

const RANDOM_TREASURES: [MapValue; 13] = [
  MapValue::SmallPickaxe,
  MapValue::LargePickaxe,
  MapValue::Drill,
  MapValue::GoldShield,
  MapValue::GoldEgg,
  MapValue::GoldPileCoins,
  MapValue::GoldBracelet,
  MapValue::GoldBar,
  MapValue::GoldCross,
  MapValue::GoldScepter,
  MapValue::GoldRubin,
  MapValue::GoldCrown,
  MapValue::Diamond,
];
const RANDOM_TREASURES_WEIGHTS: [usize; 13] = [18, 12, 8, 200, 200, 200, 200, 200, 180, 160, 140, 80, 3];

const TREASURY_TREASURES_WEIGHTS: [usize; 13] = [
  5,   // SmallPickaxe
  5,   // LargePickaxe
  5,   // Drill
  40,  // GoldShield
  80,  // GoldEgg
  200, // GoldPileCoins
  150, // GoldBracelet
  300, // GoldBar
  250, // GoldCross
  250, // GoldScepter
  250, // GoldRubin
  200, // GoldCrown
  60,  // Diamond
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CaveBiome {
  Classic = 0,
  Ice = 1,
  Magma = 2,
  Treasury = 3,
}

impl CaveBiome {
  pub fn name(&self) -> &'static str {
    match self {
      CaveBiome::Classic => "Classic Mine",
      CaveBiome::Ice => "Ice Cavern",
      CaveBiome::Magma => "Magma Core",
      CaveBiome::Treasury => "Ancient Treasury",
    }
  }

  pub fn from_u8(val: u8) -> Self {
    match val % 4 {
      0 => CaveBiome::Classic,
      1 => CaveBiome::Ice,
      2 => CaveBiome::Magma,
      _ => CaveBiome::Treasury,
    }
  }

  pub fn all() -> [CaveBiome; 4] {
    [
      CaveBiome::Classic,
      CaveBiome::Ice,
      CaveBiome::Magma,
      CaveBiome::Treasury,
    ]
  }
}

pub enum LevelInfo {
  Random,
  File { name: String, map: LevelMap },
}

impl LevelMap {
  /// Create completely empty map
  pub fn empty() -> LevelMap {
    let mut data = Vec::new();
    data.resize(usize::from(MAP_ROWS * MAP_COLS), MapValue::Passage);
    LevelMap { data }
  }

  /// Create statically typed map from a vector of bytes.
  pub fn from_file_map(external_map: Vec<u8>) -> Result<LevelMap, InvalidMap> {
    // Each map is 45 lines 66 bytes each (64 columns plus "\r\n" at the end of each row)
    if external_map.len() < 2970 {
      return Err(InvalidMap);
    }

    let mut data = Vec::with_capacity(usize::from(MAP_ROWS * MAP_COLS));
    for row in 0..MAP_ROWS {
      // Two last bytes of the row are 0xd 0xa (newline), so 64 + 2 = 66
      let row = &external_map[usize::from(row * (MAP_COLS + 2))..][..usize::from(MAP_COLS)];
      for value in row {
        // We could transmute here, but let's avoid all unsafe; amount of data is pretty small.
        data.push(MapValue::try_from(*value).unwrap());
      }
    }

    Ok(LevelMap { data })
  }

  /// Export map in the format used in map files
  #[allow(dead_code)]
  pub fn to_file_map(&self) -> Vec<u8> {
    // Each map is 45 lines 66 bytes each (64 columns plus "\r\n" at the end of each row)
    let mut data = Vec::with_capacity(usize::from(MAP_ROWS * (MAP_COLS + 2)));
    for row in 0..MAP_ROWS {
      for col in 0..MAP_COLS {
        data.push(self[row][col] as u8);
      }
      data.push(b'\r');
      data.push(b'\n');
    }
    debug_assert_eq!(data.len(), 2970);
    data
  }

  /// Generate randomized map
  pub fn random_map(treasures: u8) -> Self {
    let mut rng = rand::thread_rng();
    let seed = rng.gen::<u64>();
    let biome = CaveBiome::from_u8(rng.gen_range(0..4));
    Self::procedural_map(seed, biome, treasures, 10)
  }

  /// Generate deterministic procedural map with seed, biome and density parameters
  pub fn procedural_map(seed: u64, biome: CaveBiome, mineral_density: u8, monster_density: u8) -> Self {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut map = LevelMap::empty();
    map.generate_random_stone_seeded(&mut rng, biome);
    map.finalize_map_seeded(&mut rng, biome);
    map.generate_treasures_seeded(&mut rng, biome, mineral_density);
    map.generate_random_items_seeded(&mut rng, biome);
    if monster_density > 0 {
      map.generate_monsters_seeded(&mut rng, biome, monster_density);
    }
    map.generate_borders();
    map.generate_entrances_seeded(4, &mut rng);
    map
  }

  /// Load a campaign level for a given round
  pub fn prepare_campaign_level(game_dir: &Path, round: u16) -> Result<LevelInfo, CannotLoadSinglePlayer> {
    let filename = format!("LEVEL{}.MNL", round);
    let path = game_dir.join(filename);
    let mut map = std::fs::read(&path)
      .map_err(anyhow::Error::from)
      .and_then(|data| LevelMap::from_file_map(data).map_err(anyhow::Error::from))
      .map_err(|source| CannotLoadSinglePlayer {
        path: path.to_owned(),
        source,
      })?;

    let exit_count = Cursor::all().filter(|cur| map[*cur] == MapValue::Exit).count();
    let mut rng = rand::thread_rng();
    let selected = rng.gen_range(0..exit_count);
    let mut idx = 0;
    for cur in Cursor::all() {
      if map[cur] == MapValue::Exit {
        if idx != selected {
          map[cur] = MapValue::Passage;
        }
        idx += 1;
      }
    }
    Ok(LevelInfo::File {
      name: format!("LEVEL{}", round),
      map,
    })
  }

  /// Generate random stones on the map.
  #[allow(dead_code)]
  fn generate_random_stone(&mut self) {
    let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
    self.generate_random_stone_seeded(&mut rng, CaveBiome::Classic);
  }

  fn generate_random_stone_seeded(&mut self, rng: &mut StdRng, biome: CaveBiome) {
    let chunks = match biome {
      CaveBiome::Classic => rng.gen_range(29..40),
      CaveBiome::Ice => rng.gen_range(26..36),
      CaveBiome::Magma => rng.gen_range(32..44),
      CaveBiome::Treasury => rng.gen_range(28..38),
    };
    for _ in 0..chunks {
      self.generate_stone_chunk_seeded(rng);
    }
  }

  /// Generate one single stone chunk
  #[allow(dead_code)]
  fn generate_stone_chunk(&mut self) {
    let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
    self.generate_stone_chunk_seeded(&mut rng);
  }

  fn generate_stone_chunk_seeded(&mut self, rng: &mut StdRng) {
    let mut col = rng.gen_range(1..(MAP_COLS - 1));
    let mut row = rng.gen_range(1..(MAP_ROWS - 1));
    loop {
      match rng.gen_range(0..10) {
        0 => {
          self[row][col] = MapValue::Stone1;
        }
        1 => {
          self[row][col] = MapValue::Stone1;
          self[row + 1][col] = MapValue::Stone1;
        }
        2 => {
          self[row][col] = MapValue::Stone1;
          self[row - 1][col] = MapValue::Stone1;
        }
        3 => {
          self[row][col] = MapValue::Stone1;
          self[row][col + 1] = MapValue::Stone1;
        }
        4 => {
          self[row][col] = MapValue::Stone1;
          self[row - 1][col] = MapValue::Stone1;
          self[row + 1][col] = MapValue::Stone1;
        }
        5 => {
          self[row][col] = MapValue::Stone1;
          self[row - 1][col] = MapValue::Stone1;
          self[row + 1][col] = MapValue::Stone1;
          self[row][col - 1] = MapValue::Stone1;
        }
        6 => {
          self[row][col] = MapValue::Stone1;
          self[row - 1][col] = MapValue::Stone1;
          self[row + 1][col] = MapValue::Stone1;
          self[row][col - 1] = MapValue::Stone1;
          self[row][col + 1] = MapValue::Stone1;
        }
        7 => {
          self[row][col] = MapValue::Stone1;
          self[row - 1][col] = MapValue::Stone1;
          self[row + 1][col] = MapValue::Stone1;
          self[row][col - 1] = MapValue::Stone1;
          self[row][col + 1] = MapValue::Stone1;
        }
        8 => {
          self[row - 1][col] = MapValue::Stone1;
          self[row + 1][col] = MapValue::Stone1;
          self[row][col - 1] = MapValue::Stone1;
          self[row - 1][col - 1] = MapValue::Stone1;
          self[row + 1][col + 1] = MapValue::Stone1;
          self[row + 1][col - 1] = MapValue::Stone1;
          self[row - 1][col + 1] = MapValue::Stone1;
        }
        9 => {
          self[row][col] = MapValue::Stone1;
          self[row - 1][col] = MapValue::Stone1;
          self[row + 1][col] = MapValue::Stone1;
          self[row][col - 1] = MapValue::Stone1;
          self[row][col + 1] = MapValue::Stone1;
          self[row - 1][col - 1] = MapValue::Stone1;
          self[row + 1][col + 1] = MapValue::Stone1;
          self[row + 1][col - 1] = MapValue::Stone1;
          self[row - 1][col + 1] = MapValue::Stone1;
        }
        _ => {}
      }

      // Randomized exit condition
      if rng.gen_range(0..100) > rng.gen_range(93..103) {
        break;
      }

      row = random_offset_seeded(row, MAP_ROWS, rng);
      col = random_offset_seeded(col, MAP_COLS, rng);
    }
  }

  /// Finalize stone corners, randomize stones and sand
  #[allow(dead_code)]
  fn finalize_map(&mut self) {
    let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
    self.finalize_map_seeded(&mut rng, CaveBiome::Classic);
  }

  fn finalize_map_seeded(&mut self, rng: &mut StdRng, biome: CaveBiome) {
    // Step 1: replace lonely stones with boulders
    for cursor in Cursor::all_without_borders() {
      if self[cursor].is_stone_like()
        && self[cursor.to(Direction::Right)] == MapValue::Passage
        && self[cursor.to(Direction::Left)] == MapValue::Passage
        && self[cursor.to(Direction::Up)] == MapValue::Passage
        && self[cursor.to(Direction::Down)] == MapValue::Passage
      {
        self[cursor] = MapValue::Boulder;
      }
    }

    // Step 2: replace certain patterns of sand with rounded stone corners
    for cursor in Cursor::all_without_borders() {
      if self[cursor] == MapValue::Passage {
        if self[cursor.to(Direction::Right)] == MapValue::Stone1
          && self[cursor.to(Direction::Down)] == MapValue::Stone1
          && self[cursor.to(Direction::Left)] == MapValue::Passage
          && self[cursor.to(Direction::Up)] == MapValue::Passage
        {
          self[cursor] = MapValue::StoneTopLeft;
        } else if self[cursor.to(Direction::Right)] == MapValue::Stone1
          && self[cursor.to(Direction::Down)] == MapValue::Passage
          && self[cursor.to(Direction::Left)] == MapValue::Passage
          && self[cursor.to(Direction::Up)] == MapValue::Stone1
        {
          self[cursor] = MapValue::StoneBottomLeft;
        } else if self[cursor.to(Direction::Right)] == MapValue::Passage
          && self[cursor.to(Direction::Down)] == MapValue::Stone1
          && self[cursor.to(Direction::Left)] == MapValue::Stone1
          && self[cursor.to(Direction::Up)] == MapValue::Passage
        {
          self[cursor] = MapValue::StoneTopRight;
        } else if self[cursor.to(Direction::Right)] == MapValue::Passage
          && self[cursor.to(Direction::Down)] == MapValue::Passage
          && self[cursor.to(Direction::Left)] == MapValue::Stone1
          && self[cursor.to(Direction::Up)] == MapValue::Stone1
        {
          self[cursor] = MapValue::StoneBottomRight;
        }
      }
    }

    // Step 3: round stone corners
    for cursor in Cursor::all_without_borders() {
      if self[cursor] == MapValue::Stone1 {
        if self[cursor.to(Direction::Right)].is_stone_like()
          && self[cursor.to(Direction::Down)].is_stone_like()
          && self[cursor.to(Direction::Left)] == MapValue::Passage
          && self[cursor.to(Direction::Up)] == MapValue::Passage
        {
          self[cursor] = MapValue::StoneTopLeft;
        } else if self[cursor.to(Direction::Right)].is_stone_like()
          && self[cursor.to(Direction::Down)] == MapValue::Passage
          && self[cursor.to(Direction::Left)] == MapValue::Passage
          && self[cursor.to(Direction::Up)].is_stone_like()
        {
          self[cursor] = MapValue::StoneBottomLeft;
        } else if self[cursor.to(Direction::Right)] == MapValue::Passage
          && self[cursor.to(Direction::Down)].is_stone_like()
          && self[cursor.to(Direction::Left)].is_stone_like()
          && self[cursor.to(Direction::Up)] == MapValue::Passage
        {
          self[cursor] = MapValue::StoneTopRight;
        } else if self[cursor.to(Direction::Right)] == MapValue::Passage
          && self[cursor.to(Direction::Down)] == MapValue::Passage
          && self[cursor.to(Direction::Left)].is_stone_like()
          && self[cursor.to(Direction::Up)].is_stone_like()
        {
          self[cursor] = MapValue::StoneBottomRight;
        }
      }
    }

    // Step 4: randomize sand and stone by biome
    for cursor in Cursor::all() {
      if self[cursor] == MapValue::Stone1 {
        self[cursor] = match biome {
          CaveBiome::Ice => {
            if rng.gen_bool(0.4) {
              MapValue::StoneLightCracked
            } else {
              *[MapValue::Stone1, MapValue::Stone4].choose(rng).unwrap()
            }
          }
          CaveBiome::Magma => {
            if rng.gen_bool(0.4) {
              MapValue::StoneHeavyCracked
            } else if rng.gen_bool(0.3) {
              MapValue::StoneLightCracked
            } else {
              MapValue::Stone3
            }
          }
          CaveBiome::Treasury => {
            if rng.gen_bool(0.45) {
              MapValue::Brick
            } else if rng.gen_bool(0.2) {
              MapValue::BrickLightCracked
            } else {
              *[MapValue::Stone1, MapValue::Stone2].choose(rng).unwrap()
            }
          }
          CaveBiome::Classic => {
            *[MapValue::Stone1, MapValue::Stone2, MapValue::Stone3, MapValue::Stone4]
              .choose(rng)
              .unwrap()
          }
        };
      } else if self[cursor] == MapValue::Passage {
        self[cursor] = match biome {
          CaveBiome::Ice => {
            if rng.gen_bool(0.6) {
              MapValue::LightGravel
            } else {
              MapValue::Sand1
            }
          }
          CaveBiome::Magma => {
            if rng.gen_bool(0.7) {
              MapValue::Sand3
            } else {
              MapValue::Sand2
            }
          }
          CaveBiome::Treasury => {
            if rng.gen_bool(0.5) {
              MapValue::Sand1
            } else {
              MapValue::Passage
            }
          }
          CaveBiome::Classic => {
            *[MapValue::Sand1, MapValue::Sand2, MapValue::Sand3]
              .choose(rng)
              .unwrap()
          }
        };
      }
    }

    // Step 5: place gravel / hazards by biome
    match biome {
      CaveBiome::Ice => {
        for _ in 0..200 {
          let cursor = self.pick_random_coord_seeded(MapValue::is_sand, rng);
          self[cursor] = MapValue::LightGravel;
        }
      }
      CaveBiome::Magma => {
        for _ in 0..200 {
          let cursor = self.pick_random_coord_seeded(MapValue::is_sand, rng);
          self[cursor] = MapValue::HeavyGravel;
        }
        let lava_pools = rng.gen_range(12..25);
        for _ in 0..lava_pools {
          let cursor = self.pick_random_coord_seeded(MapValue::is_sand, rng);
          self[cursor] = MapValue::Napalm1;
        }
      }
      CaveBiome::Treasury => {
        for _ in 0..100 {
          let cursor = self.pick_random_coord_seeded(MapValue::is_sand, rng);
          self[cursor] = MapValue::HeavyGravel;
        }
      }
      CaveBiome::Classic => {
        for _ in 0..300 {
          let cursor = self.pick_random_coord_seeded(MapValue::is_sand, rng);
          self[cursor] = *[MapValue::LightGravel, MapValue::HeavyGravel].choose(rng).unwrap();
        }
      }
    }
  }

  /// Place treasures on the map
  #[allow(dead_code)]
  fn generate_treasures(&mut self, treasures: u8) {
    let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
    self.generate_treasures_seeded(&mut rng, CaveBiome::Classic, treasures);
  }

  fn generate_treasures_seeded(&mut self, rng: &mut StdRng, biome: CaveBiome, mineral_density: u8) {
    let weights = match biome {
      CaveBiome::Treasury => &TREASURY_TREASURES_WEIGHTS,
      _ => &RANDOM_TREASURES_WEIGHTS,
    };
    let distribution = WeightedIndex::new(weights).unwrap();

    let count_base = if mineral_density > 0 { mineral_density as usize } else { 40 };
    let treasure_count = match biome {
      CaveBiome::Treasury => (count_base * 3 / 2).max(45),
      _ => count_base,
    };

    let mut treasures_in_stone = 0;
    for _ in 0..treasure_count {
      let item = RANDOM_TREASURES[distribution.sample(rng)];
      if treasures_in_stone > 20 {
        let col = rng.gen_range(1..(MAP_COLS - 1));
        let row = rng.gen_range(1..(MAP_ROWS - 1));
        self[Cursor::new(row, col)] = item;
      } else {
        let cursor = self.pick_random_coord_seeded(MapValue::is_stone, rng);
        self[cursor] = item;
        treasures_in_stone += 1;
      }
    }

    match biome {
      CaveBiome::Ice => {
        let freeze_bombs = rng.gen_range(4..9);
        for _ in 0..freeze_bombs {
          let cursor = self.pick_random_coord_seeded(MapValue::is_stone, rng);
          self[cursor] = MapValue::FreezeBomb;
        }
      }
      CaveBiome::Magma => {
        let explosives = rng.gen_range(3..7);
        for _ in 0..explosives {
          let cursor = self.pick_random_coord_seeded(MapValue::is_stone, rng);
          self[cursor] = MapValue::Dynamite1;
        }
      }
      CaveBiome::Treasury => {
        let diamonds = rng.gen_range(2..5);
        for _ in 0..diamonds {
          let cursor = self.pick_random_coord_seeded(MapValue::is_stone, rng);
          self[cursor] = MapValue::Diamond;
        }
      }
      _ => {}
    }
  }

  /// Generate various random items
  #[allow(dead_code)]
  fn generate_random_items(&mut self) {
    let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
    self.generate_random_items_seeded(&mut rng, CaveBiome::Classic);
  }

  fn generate_random_items_seeded(&mut self, rng: &mut StdRng, biome: CaveBiome) {
    while rng.gen_range(0..100) > 70 {
      let coord = random_coord_seeded(rng);
      self[coord] = MapValue::Boulder;
    }

    while rng.gen_range(0..100) > 70 {
      let coord = random_coord_seeded(rng);
      self[coord] = MapValue::WeaponsCrate;
    }

    while rng.gen_range(0..100) > 65 {
      let coord = random_coord_seeded(rng);
      self[coord] = MapValue::Medikit;
    }

    while rng.gen_range(0..100) > 70 {
      let c1 = random_coord_seeded(rng);
      let c2 = random_coord_seeded(rng);
      self[c1] = MapValue::Teleport;
      self[c2] = MapValue::Teleport;
    }

    if biome == CaveBiome::Magma {
      let barrels = rng.gen_range(4..10);
      for _ in 0..barrels {
        let cursor = self.pick_random_coord_seeded(MapValue::is_passable, rng);
        self[cursor] = MapValue::Barrel;
      }
    }
  }

  fn generate_monsters_seeded(&mut self, rng: &mut StdRng, biome: CaveBiome, monster_density: u8) {
    let count = ((monster_density as usize + 7) / 8).clamp(2, 16);
    let monster_options: &[MapValue] = match biome {
      CaveBiome::Ice => &[
        MapValue::SlimeRight,
        MapValue::SlimeLeft,
        MapValue::SlimeUp,
        MapValue::SlimeDown,
        MapValue::FurryRight,
        MapValue::FurryLeft,
      ],
      CaveBiome::Magma => &[
        MapValue::GrenadierRight,
        MapValue::GrenadierLeft,
        MapValue::AlienRight,
        MapValue::AlienLeft,
      ],
      CaveBiome::Treasury => &[
        MapValue::AlienRight,
        MapValue::AlienLeft,
        MapValue::AlienUp,
        MapValue::AlienDown,
        MapValue::GrenadierRight,
        MapValue::GrenadierLeft,
      ],
      CaveBiome::Classic => &[
        MapValue::FurryRight,
        MapValue::FurryLeft,
        MapValue::GrenadierRight,
        MapValue::GrenadierLeft,
        MapValue::SlimeRight,
        MapValue::SlimeLeft,
        MapValue::AlienRight,
        MapValue::AlienLeft,
      ],
    };

    for _ in 0..count {
      let mut placed = false;
      for _ in 0..20 {
        let row = rng.gen_range(5..(MAP_ROWS - 5));
        let col = rng.gen_range(5..(MAP_COLS - 5));
        let cur = Cursor::new(row, col);
        if self[cur].is_passable() || self[cur].is_sand() {
          let m = *monster_options.choose(rng).unwrap();
          self[cur] = m;
          placed = true;
          break;
        }
      }
      if !placed {
        let cur = self.pick_random_coord_seeded(MapValue::is_sand, rng);
        let m = *monster_options.choose(rng).unwrap();
        self[cur] = m;
      }
    }
  }

  /// Generate metal borders around the map
  fn generate_borders(&mut self) {
    for row in 0..MAP_ROWS {
      self[row][0] = MapValue::MetalWall;
      self[row][MAP_COLS - 1] = MapValue::MetalWall;
    }

    for col in 0..MAP_COLS {
      self[0][col] = MapValue::MetalWall;
      self[MAP_ROWS - 1][col] = MapValue::MetalWall;
    }
  }

  /// Pick random coordinate such that its map value matches the predicate. Returns row and column.
  #[allow(dead_code)]
  fn pick_random_coord(&self, predicate: impl Fn(MapValue) -> bool) -> Cursor {
    let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
    self.pick_random_coord_seeded(predicate, &mut rng)
  }

  fn pick_random_coord_seeded(&self, predicate: impl Fn(MapValue) -> bool, rng: &mut StdRng) -> Cursor {
    let mut cursor = random_coord_seeded(rng);
    for _ in 0..(MAP_ROWS * MAP_COLS) {
      if predicate(self[cursor]) {
        break;
      }

      if cursor.col < MAP_COLS - 2 {
        cursor.col += 1;
      } else {
        cursor.col = 1;
        cursor.row += 1;
      }
      if cursor.row >= MAP_ROWS - 1 {
        cursor = random_coord_seeded(rng);
      }
    }
    cursor
  }

  pub fn generate_entrances(&mut self, players: u8) {
    let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
    self.generate_entrances_seeded(players, &mut rng);
  }

  pub fn generate_entrances_seeded(&mut self, players: u8, rng: &mut StdRng) {
    // Top left
    let rnd = rng.gen_range(4..10);
    for col in 1..=rnd {
      self[1][col] = MapValue::Passage;
    }
    let rnd = rng.gen_range(4..10);
    for row in 1..=rnd {
      self[row][1] = MapValue::Passage;
    }

    // Bottom right
    let rnd = rng.gen_range(4..10);
    for col in 1..=rnd {
      self[MAP_ROWS - 2][MAP_COLS - 1 - col] = MapValue::Passage;
    }
    let rnd = rng.gen_range(4..10);
    for row in 1..=rnd {
      self[MAP_ROWS - 1 - row][MAP_COLS - 2] = MapValue::Passage;
    }

    if players > 2 {
      // Top right
      let rnd = rng.gen_range(4..10);
      for col in 1..=rnd {
        self[1][MAP_COLS - 1 - col] = MapValue::Passage;
      }
      let rnd = rng.gen_range(4..10);
      for row in 1..=rnd {
        self[row][MAP_COLS - 2] = MapValue::Passage;
      }

      // Bottom left
      let rnd = rng.gen_range(4..10);
      for col in 1..=rnd {
        self[MAP_ROWS - 2][col] = MapValue::Passage;
      }
      let rnd = rng.gen_range(4..10);
      for row in 1..=rnd {
        self[MAP_ROWS - 1 - row][1] = MapValue::Passage;
      }
    }
  }
}

#[allow(dead_code)]
fn random_coord() -> Cursor {
  let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
  random_coord_seeded(&mut rng)
}

fn random_coord_seeded(rng: &mut StdRng) -> Cursor {
  let col = rng.gen_range(1..(MAP_COLS - 1));
  let row = rng.gen_range(1..(MAP_ROWS - 1));
  Cursor::new(row, col)
}

/// Enum for all possible map values.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, PartialOrd)]
pub enum MapValue {
  Map00 = 0x00,
  Map01 = 0x01,
  Map02 = 0x02,
  Map03 = 0x03,
  Map04 = 0x04,
  Map05 = 0x05,
  Map06 = 0x06,
  Map07 = 0x07,
  Map08 = 0x08,
  Map09 = 0x09,
  Map0A = 0x0A,
  Map0B = 0x0B,
  Map0C = 0x0C,
  Map0D = 0x0D,
  Map0E = 0x0E,
  Map0F = 0x0F,
  Map10 = 0x10,
  Map11 = 0x11,
  Map12 = 0x12,
  Map13 = 0x13,
  Map14 = 0x14,
  Map15 = 0x15,
  Map16 = 0x16,
  Map17 = 0x17,
  Map18 = 0x18,
  Map19 = 0x19,
  Map1A = 0x1A,
  Map1B = 0x1B,
  Map1C = 0x1C,
  Map1D = 0x1D,
  Map1E = 0x1E,
  Map1F = 0x1F,
  Map20 = 0x20,
  Map21 = 0x21,
  Map22 = 0x22,
  Map23 = 0x23,
  Map24 = 0x24,
  Map25 = 0x25,
  Map26 = 0x26,
  Map27 = 0x27,
  Map28 = 0x28,
  Map29 = 0x29,
  Map2A = 0x2A,
  Map2B = 0x2B,
  Map2C = 0x2C,
  Map2D = 0x2D,
  Map2E = 0x2E,
  Map2F = 0x2F,
  Passage = 0x30,
  MetalWall = 0x31,
  Sand1 = 0x32,
  Sand2 = 0x33,
  Sand3 = 0x34,
  LightGravel = 0x35,
  HeavyGravel = 0x36,
  StoneTopLeft = 0x37,
  StoneTopRight = 0x38,
  StoneBottomRight = 0x39,
  Map3A = 0x3A,
  Map3B = 0x3B,
  Map3C = 0x3C,
  Map3D = 0x3D,
  Map3E = 0x3E,
  Map3F = 0x3F,
  Map40 = 0x40,
  StoneBottomLeft = 0x41,
  Boulder = 0x42,
  Stone1 = 0x43,
  Stone2 = 0x44,
  Stone3 = 0x45,
  Stone4 = 0x46,
  FurryRight = 0x47,
  FurryLeft = 0x48,
  FurryUp = 0x49,
  FurryDown = 0x4A,
  GrenadierRight = 0x4B,
  GrenadierLeft = 0x4C,
  GrenadierUp = 0x4D,
  GrenadierDown = 0x4E,
  SlimeRight = 0x4F,
  SlimeLeft = 0x50,
  SlimeUp = 0x51,
  SlimeDown = 0x52,
  AlienRight = 0x53,
  AlienLeft = 0x54,
  AlienUp = 0x55,
  AlienDown = 0x56,
  SmallBomb1 = 0x57,
  BigBomb1 = 0x58,
  Dynamite1 = 0x59,
  /// Same as TempMarker2, but used for napalm
  NapalmTempMarker2 = 0x5A,
  BlackHoleBomb = 0x5B,
  BlackHoleActive = 0x5C,
  FreezeBomb = 0x5D,
  Map5E = 0x5E,
  Map5F = 0x5F,
  Map60 = 0x60,
  Smoke1 = 0x61,
  Smoke2 = 0x62,
  SmallRadioBlue = 0x63,
  BigRadioBlue = 0x64,
  Mine = 0x65,
  Blood = 0x66,
  SmallRadioGreen = 0x67,
  BigRadioGreen = 0x68,
  SmallRadioYellow = 0x69,
  BigRadioYellow = 0x6A,
  Exit = 0x6B,
  Door = 0x6C,
  Medikit = 0x6D,
  Map6E = 0x6E,
  Biomass = 0x6F,
  StoneLightCracked = 0x70,
  StoneHeavyCracked = 0x71,
  Map72 = 0x72,
  Diamond = 0x73,
  Map74 = 0x74,
  Map75 = 0x75,
  Map76 = 0x76,
  SmallBomb2 = 0x77,
  SmallBomb3 = 0x78,
  WeaponsCrate = 0x79,
  /// Same as TempMarker1, but used for napalm
  NapalmTempMarker1 = 0x7A,
  Map7B = 0x7B,
  NapalmExtinguished = 0x7C,
  SmallBombExtinguished = 0x7D,
  BigBombExtinguished = 0x7E,
  Napalm1 = 0x7F,
  LargeCrucifixBomb = 0x80,
  PlasticBomb = 0x81,
  SmallRadioRed = 0x82,
  BigRadioRed = 0x83,
  Explosion = 0x84,
  MonsterDying = 0x85,
  MonsterSmoke1 = 0x86,
  MonsterSmoke2 = 0x87,
  /// Temporary value used in plastic and digger spreading algorithm
  TempMarker1 = 0x88,
  /// Temporary value used in plastic and digger spreading algorithm
  TempMarker2 = 0x89,
  SmallCrucifixBomb = 0x8A,
  BigBomb2 = 0x8B,
  BigBomb3 = 0x8C,
  Dynamite2 = 0x8D,
  Dynamite3 = 0x8E,
  SmallPickaxe = 0x8F,
  LargePickaxe = 0x90,
  Drill = 0x91,
  GoldShield = 0x92,
  GoldEgg = 0x93,
  GoldPileCoins = 0x94,
  GoldBracelet = 0x95,
  GoldBar = 0x96,
  GoldCross = 0x97,
  GoldScepter = 0x98,
  GoldRubin = 0x99,
  GoldCrown = 0x9A,
  Plastic = 0x9B,
  Teleport = 0x9C,
  Atomic1 = 0x9D,
  Atomic2 = 0x9E,
  Atomic3 = 0x9F,
  ExplosivePlastic = 0xA0,
  ExplosivePlasticBomb = 0xA1,
  DiggerBomb = 0xA2,
  Napalm2 = 0xA3,
  Barrel = 0xA4,
  GrenadeFlyingRight = 0xA5,
  GrenadeFlyingLeft = 0xA6,
  GrenadeFlyingDown = 0xA7,
  GrenadeFlyingUp = 0xA8,
  MetalWallPlaced = 0xA9,
  DynamiteExtinguished = 0xAA,
  JumpingBomb = 0xAB,
  Brick = 0xAC,
  BrickLightCracked = 0xAD,
  BrickHeavyCracked = 0xAE,
  SlimeCorpse = 0xAF,
  SlimeDying = 0xB0,
  SlimeSmoke1 = 0xB1,
  SlimeSmoke2 = 0xB2,
  LifeItem = 0xB3,
  ButtonOff = 0xB4,
  ButtonOn = 0xB5,
  Item182 = 0xB6,
  MapB7 = 0xB7,
  MapB8 = 0xB8,
  MapB9 = 0xB9,
  MapBA = 0xBA,
  MapBB = 0xBB,
  MapBC = 0xBC,
  MapBD = 0xBD,
  MapBE = 0xBE,
  MapBF = 0xBF,
  MapC0 = 0xC0,
  MapC1 = 0xC1,
  MapC2 = 0xC2,
  MapC3 = 0xC3,
  MapC4 = 0xC4,
  MapC5 = 0xC5,
  MapC6 = 0xC6,
  MapC7 = 0xC7,
  MapC8 = 0xC8,
  MapC9 = 0xC9,
  MapCA = 0xCA,
  MapCB = 0xCB,
  MapCC = 0xCC,
  MapCD = 0xCD,
  MapCE = 0xCE,
  MapCF = 0xCF,
  DrillDroneRight = 0xD0,
  DrillDroneLeft = 0xD1,
  DrillDroneUp = 0xD2,
  DrillDroneDown = 0xD3,
  MapD4 = 0xD4,
  MapD5 = 0xD5,
  MapD6 = 0xD6,
  MapD7 = 0xD7,
  MapD8 = 0xD8,
  MapD9 = 0xD9,
  MapDA = 0xDA,
  MapDB = 0xDB,
  MapDC = 0xDC,
  MapDD = 0xDD,
  MapDE = 0xDE,
  MapDF = 0xDF,
  MapE0 = 0xE0,
  MapE1 = 0xE1,
  MapE2 = 0xE2,
  MapE3 = 0xE3,
  MapE4 = 0xE4,
  MapE5 = 0xE5,
  MapE6 = 0xE6,
  MapE7 = 0xE7,
  MapE8 = 0xE8,
  MapE9 = 0xE9,
  MapEA = 0xEA,
  MapEB = 0xEB,
  MapEC = 0xEC,
  MapED = 0xED,
  MapEE = 0xEE,
  MapEF = 0xEF,
  MapF0 = 0xF0,
  MapF1 = 0xF1,
  MapF2 = 0xF2,
  MapF3 = 0xF3,
  MapF4 = 0xF4,
  MapF5 = 0xF5,
  MapF6 = 0xF6,
  MapF7 = 0xF7,
  MapF8 = 0xF8,
  MapF9 = 0xF9,
  MapFA = 0xFA,
  MapFB = 0xFB,
  MapFC = 0xFC,
  MapFD = 0xFD,
  MapFE = 0xFE,
  MapFF = 0xFF,
}

impl MapValue {
  /// Check if value is stone or any of the stone corners
  pub fn is_stone_like(self) -> bool {
    matches!(
      self,
      MapValue::Stone1
        | MapValue::Stone2
        | MapValue::Stone3
        | MapValue::Stone4
        | MapValue::StoneTopLeft
        | MapValue::StoneTopRight
        | MapValue::StoneBottomLeft
        | MapValue::StoneBottomRight
        | MapValue::StoneLightCracked
        | MapValue::StoneHeavyCracked
    )
  }

  /// Check if cell is a stone corner
  pub fn is_stone_corner(self) -> bool {
    matches!(
      self,
      MapValue::StoneTopLeft | MapValue::StoneTopRight | MapValue::StoneBottomLeft | MapValue::StoneBottomRight
    )
  }

  /// Check if value is stone
  pub fn is_stone(self) -> bool {
    matches!(
      self,
      MapValue::Stone1 | MapValue::Stone2 | MapValue::Stone3 | MapValue::Stone4
    )
  }

  /// Check if value is sand
  pub fn is_sand(self) -> bool {
    matches!(self, MapValue::Sand1 | MapValue::Sand2 | MapValue::Sand3)
  }

  pub fn is_brick_like(self) -> bool {
    matches!(
      self,
      MapValue::Brick | MapValue::BrickLightCracked | MapValue::BrickHeavyCracked
    )
  }

  /// Check if value is passable square
  pub fn is_passable(self) -> bool {
    matches!(self, MapValue::Passage | MapValue::Blood | MapValue::SlimeCorpse)
  }

  /// If map value is a monster, return its actor kind and direction.
  pub fn monster(self) -> Option<(ActorKind, Direction)> {
    Some(match self {
      MapValue::FurryRight => (ActorKind::Furry, Direction::Right),
      MapValue::FurryLeft => (ActorKind::Furry, Direction::Left),
      MapValue::FurryUp => (ActorKind::Furry, Direction::Up),
      MapValue::FurryDown => (ActorKind::Furry, Direction::Down),

      MapValue::GrenadierRight => (ActorKind::Grenadier, Direction::Right),
      MapValue::GrenadierLeft => (ActorKind::Grenadier, Direction::Left),
      MapValue::GrenadierUp => (ActorKind::Grenadier, Direction::Up),
      MapValue::GrenadierDown => (ActorKind::Grenadier, Direction::Down),

      MapValue::SlimeRight => (ActorKind::Slime, Direction::Right),
      MapValue::SlimeLeft => (ActorKind::Slime, Direction::Left),
      MapValue::SlimeUp => (ActorKind::Slime, Direction::Up),
      MapValue::SlimeDown => (ActorKind::Slime, Direction::Down),

      MapValue::AlienRight => (ActorKind::Alien, Direction::Right),
      MapValue::AlienLeft => (ActorKind::Alien, Direction::Left),
      MapValue::AlienUp => (ActorKind::Alien, Direction::Up),
      MapValue::AlienDown => (ActorKind::Alien, Direction::Down),
      _ => return None,
    })
  }

  /// Return gold value of the map cell
  pub fn gold_value(self) -> u32 {
    match self {
      MapValue::GoldShield => 15,
      MapValue::GoldEgg => 25,
      MapValue::GoldPileCoins => 15,
      MapValue::GoldBracelet => 10,
      MapValue::GoldBar => 30,
      MapValue::GoldCross => 35,
      MapValue::GoldScepter => 50,
      MapValue::GoldRubin => 65,
      MapValue::GoldCrown => 100,
      MapValue::Diamond => 1000,
      _ => 0,
    }
  }

  /// Check if map value is treasure item
  pub fn is_treasure(self) -> bool {
    self.gold_value() > 0
  }

  pub fn is_bomb(self) -> bool {
    matches!(
      self,
      MapValue::SmallBomb1
        | MapValue::SmallBomb2
        | MapValue::SmallBomb3
        | MapValue::BigBomb1
        | MapValue::BigBomb2
        | MapValue::BigBomb3
        | MapValue::Dynamite1
        | MapValue::Dynamite2
        | MapValue::Dynamite3
        | MapValue::Napalm1
        | MapValue::Napalm2
        | MapValue::SmallCrucifixBomb
        | MapValue::LargeCrucifixBomb
        | MapValue::PlasticBomb
        | MapValue::ExplosivePlastic
        | MapValue::ExplosivePlasticBomb
        | MapValue::Atomic1
        | MapValue::Atomic2
        | MapValue::Atomic3
        | MapValue::DiggerBomb
        | MapValue::Barrel
        | MapValue::GrenadeFlyingRight
        | MapValue::GrenadeFlyingLeft
        | MapValue::GrenadeFlyingDown
        | MapValue::GrenadeFlyingUp
        | MapValue::MetalWallPlaced
        | MapValue::JumpingBomb
        | MapValue::BlackHoleBomb
        | MapValue::BlackHoleActive
        | MapValue::FreezeBomb
        | MapValue::DrillDroneRight
        | MapValue::DrillDroneLeft
        | MapValue::DrillDroneUp
        | MapValue::DrillDroneDown
    )
  }

  pub fn is_custom_explodable(self) -> bool {
    matches!(
      self,
      MapValue::BlackHoleBomb
        | MapValue::BlackHoleActive
        | MapValue::FreezeBomb
        | MapValue::DrillDroneRight
        | MapValue::DrillDroneLeft
        | MapValue::DrillDroneUp
        | MapValue::DrillDroneDown
    )
  }
}

/// Apply random offset to the coordinate
#[allow(dead_code)]
fn random_offset(coord: u16, max: u16) -> u16 {
  let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
  random_offset_seeded(coord, max, &mut rng)
}

fn random_offset_seeded(mut coord: u16, max: u16, rng: &mut StdRng) -> u16 {
  // Note: original game uses condition `x < 1` here (for both rows and columns). We use `x < 2` so
  // we never get too close to the border that one of the offsets above go outside of the map.
  if coord < 2 {
    coord += 1;
  } else if coord >= max - 2 {
    coord -= 1;
  } else {
    // Apply random offset -1, 0 or 1.
    coord += 1;
    coord -= rng.gen_range(0..3);
  }
  coord
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_procedural_biome_generation_deterministic() {
    let seed = 123456789;
    for biome in CaveBiome::all().iter().copied() {
      let map1 = LevelMap::procedural_map(seed, biome, 20, 10);
      let map2 = LevelMap::procedural_map(seed, biome, 20, 10);
      assert_eq!(
        map1.to_file_map(),
        map2.to_file_map(),
        "Maps with identical seeds and biome must match"
      );
    }
  }

  #[test]
  fn test_procedural_biome_differences() {
    let seed = 987654321;
    let classic = LevelMap::procedural_map(seed, CaveBiome::Classic, 30, 10);
    let magma = LevelMap::procedural_map(seed, CaveBiome::Magma, 30, 10);
    let ice = LevelMap::procedural_map(seed, CaveBiome::Ice, 30, 10);
    let treasury = LevelMap::procedural_map(seed, CaveBiome::Treasury, 30, 10);

    // Magma should have napalm pools or barrels
    let magma_hazards = Cursor::all().any(|c| magma[c] == MapValue::Napalm1 || magma[c] == MapValue::Barrel);
    assert!(magma_hazards, "Magma biome must contain lava pools or barrels");

    // Ice cavern should have slick floor (LightGravel) or freeze bombs
    let ice_slick = Cursor::all().any(|c| ice[c] == MapValue::LightGravel || ice[c] == MapValue::FreezeBomb);
    assert!(ice_slick, "Ice biome must contain slick light gravel or freeze bombs");

    // Treasury should contain bricks and high value treasures
    let treasury_bricks = Cursor::all().any(|c| treasury[c].is_brick_like());
    assert!(treasury_bricks, "Treasury biome must contain ancient bricks");

    // Maps with different biomes must not be identical
    assert_ne!(classic.to_file_map(), magma.to_file_map());
    assert_ne!(ice.to_file_map(), treasury.to_file_map());
  }

  #[test]
  fn test_map_borders_and_entrances() {
    let map = LevelMap::procedural_map(42, CaveBiome::Classic, 15, 5);
    // Borders must be metal walls
    for col in 0..MAP_COLS {
      assert_eq!(map[0][col], MapValue::MetalWall);
      assert_eq!(map[MAP_ROWS - 1][col], MapValue::MetalWall);
    }
    for row in 0..MAP_ROWS {
      assert_eq!(map[row][0], MapValue::MetalWall);
      assert_eq!(map[row][MAP_COLS - 1], MapValue::MetalWall);
    }
    // Spawn corners must be clear passages
    assert_eq!(map[1][1], MapValue::Passage);
    assert_eq!(map[MAP_ROWS - 2][MAP_COLS - 2], MapValue::Passage);
    assert_eq!(map[1][MAP_COLS - 2], MapValue::Passage);
    assert_eq!(map[MAP_ROWS - 2][1], MapValue::Passage);
  }
}
