use crate::world::actor::{ActorKind, Player};
use crate::world::equipment::Equipment;
use crate::world::map::MapValue;
use crate::world::position::Direction;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationPhase {
  Phase1 = 0,
  Phase2 = 1,
  Phase3 = 2,
  Phase4 = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Digging {
  Hands,
  Pickaxe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Border {
  Burned,
  Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphRect {
  pub x: i32,
  pub y: i32,
  pub w: u32,
  pub h: u32,
}

#[derive(Clone, Copy)]
pub enum Glyph {
  ShovelPointer,
  ArrowPointer,
  RadioButton(bool),
  ShopSlot(bool),
  Selection(Equipment),
  Ready,
  Map(MapValue),
  SandBorder(Direction, Border),
  StoneBorder(Direction, Border),
  Monster(ActorKind, Direction, Digging, AnimationPhase),
  Life,
  LifeLost,
}

impl Glyph {
  pub fn rect(self) -> GlyphRect {
    let (left, top, right, bottom) = match self {
      Glyph::ShovelPointer => (150, 140, 215, 160),
      Glyph::ArrowPointer => (205, 99, 231, 109),
      Glyph::RadioButton(false) => (90, 40, 104, 52),
      Glyph::RadioButton(true) => (90, 53, 104, 65),
      Glyph::ShopSlot(false) => (64, 92, 127, 139),
      Glyph::ShopSlot(true) => (128, 92, 191, 139),
      Glyph::Ready => (120, 140, 149, 169),
      Glyph::Selection(equipment) => {
        let (x, y) = EQUIPMENT_GLYPHS[equipment as usize];
        (x, y, x + 29, y + 29)
      }
      Glyph::Map(value) => {
        let (x, y) = if value >= MapValue::Passage && value <= MapValue::Item182 {
          MAP_GLYPHS[(value as usize) - (MapValue::Passage as usize)]
        } else {
          UNMAPPED
        };
        (x, y, x + 9, y + 9)
      }
      Glyph::SandBorder(Direction::Left, Border::Normal) => (194, 98, 197, 107),
      Glyph::SandBorder(Direction::Right, Border::Normal) => (200, 98, 203, 107),
      Glyph::SandBorder(Direction::Up, Border::Normal) => (194, 109, 203, 111),
      Glyph::SandBorder(Direction::Down, Border::Normal) => (194, 113, 203, 115),

      Glyph::StoneBorder(Direction::Left, Border::Normal) => (148, 60, 151, 69),
      Glyph::StoneBorder(Direction::Right, Border::Normal) => (154, 60, 157, 69),
      Glyph::StoneBorder(Direction::Up, Border::Normal) => (148, 71, 157, 73),
      Glyph::StoneBorder(Direction::Down, Border::Normal) => (148, 75, 157, 77),

      Glyph::SandBorder(Direction::Left, Border::Burned) => (194, 117, 197, 126),
      Glyph::SandBorder(Direction::Right, Border::Burned) => (200, 117, 203, 126),
      Glyph::SandBorder(Direction::Up, Border::Burned) => (194, 128, 203, 130),
      Glyph::SandBorder(Direction::Down, Border::Burned) => (194, 132, 203, 134),

      Glyph::StoneBorder(Direction::Left, Border::Burned) => (205, 117, 208, 126),
      Glyph::StoneBorder(Direction::Right, Border::Burned) => (211, 117, 214, 126),
      Glyph::StoneBorder(Direction::Up, Border::Burned) => (205, 128, 214, 130),
      Glyph::StoneBorder(Direction::Down, Border::Burned) => (205, 132, 214, 134),

      Glyph::Monster(kind, dir, digging, anim) => {
        let anim = anim as u8;
        let (pos_x, pos_y) = match kind {
          ActorKind::Furry => (160, 50),
          ActorKind::Grenadier => (160, 60),
          ActorKind::Slime => (160, 70),
          ActorKind::Alien => (0, 80),
          ActorKind::Player(Player::Player1) | ActorKind::Clone(Player::Player1) if digging == Digging::Pickaxe => {
            (160, 200)
          }
          ActorKind::Player(Player::Player2) | ActorKind::Clone(Player::Player2) if digging == Digging::Pickaxe => {
            (0, 200)
          }
          ActorKind::Player(Player::Player3) | ActorKind::Clone(Player::Player3) if digging == Digging::Pickaxe => {
            (0, 210)
          }
          ActorKind::Player(Player::Player4) | ActorKind::Clone(Player::Player4) if digging == Digging::Pickaxe => {
            (160, 210)
          }
          ActorKind::Player(Player::Player1) | ActorKind::Clone(Player::Player1) => (160, 10),
          ActorKind::Player(Player::Player2) | ActorKind::Clone(Player::Player2) => (160, 0),
          ActorKind::Player(Player::Player3) | ActorKind::Clone(Player::Player3) => (160, 30),
          ActorKind::Player(Player::Player4) | ActorKind::Clone(Player::Player4) => (160, 40),
        };
        let pos_x = pos_x + (dir as i16) * 40 + i16::from(anim) * 10;
        (pos_x, pos_y, pos_x + 9, pos_y + 9)
      }
      Glyph::Life => (31, 91, 42, 111),
      Glyph::LifeLost => (43, 91, 61, 111),
    };
    GlyphRect {
      x: i32::from(left),
      y: i32::from(top),
      w: (right - left + 1) as u32,
      h: (bottom - top + 1) as u32,
    }
  }
}

pub const EQUIPMENT_GLYPHS: [(i16, i16); Equipment::TOTAL] = [
  (0, 170),
  (30, 170),
  (60, 170),
  (216, 140),
  (240, 170),
  (210, 170),
  (246, 140),
  (270, 170),
  (90, 170),
  (120, 170),
  (246, 110),
  (90, 140),
  (150, 170),
  (180, 170),
  (276, 140),
  (276, 110),
  (216, 110),
  (0, 140),
  (30, 140),
  (60, 140),
  (30, 40),
  (232, 80),
  (262, 80),
  (0, 40),
  (105, 40),
  (60, 40),
  (0, 90),
];

pub const UNMAPPED: (i16, i16) = (50, 70);

pub const MAP_GLYPHS: [(i16, i16); 135] = [
  (0, 0),
  (10, 0),
  (20, 0),
  (30, 0),
  (40, 0),
  (50, 0),
  (60, 0),
  (70, 0),
  (80, 0),
  (90, 0),
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  (100, 0),
  (110, 0),
  (120, 0),
  (130, 0),
  (140, 0),
  (150, 0),
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  (0, 10),
  (10, 10),
  (20, 10),
  (90, 10),
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  (100, 10),
  (110, 10),
  (120, 10),
  (130, 10),
  (140, 10),
  (150, 10),
  (20, 30),
  (30, 30),
  (40, 30),
  (50, 30),
  (0, 30),
  (10, 30),
  (60, 30),
  UNMAPPED,
  (70, 30),
  (100, 30),
  (90, 30),
  UNMAPPED,
  (150, 30),
  UNMAPPED,
  UNMAPPED,
  UNMAPPED,
  (0, 20),
  (10, 20),
  (20, 20),
  UNMAPPED,
  UNMAPPED,
  (110, 30),
  (120, 30),
  (130, 30),
  (40, 10),
  (50, 10),
  (60, 10),
  (70, 10),
  (80, 10),
  (90, 10),
  (90, 10),
  (100, 10),
  (110, 10),
  UNMAPPED,
  (90, 10),
  (50, 20),
  (60, 20),
  (70, 20),
  (80, 20),
  (90, 20),
  (100, 20),
  (110, 20),
  (120, 20),
  (130, 20),
  (140, 20),
  (150, 20),
  (160, 20),
  (170, 20),
  (180, 20),
  (190, 20),
  (200, 20),
  (210, 20),
  (220, 20),
  (230, 20),
  (240, 20),
  (250, 20),
  (260, 20),
  (270, 20),
  (280, 20),
  (290, 20),
  (40, 20),
  (300, 20),
  (310, 20),
  (310, 20),
  (310, 20),
  (310, 20),
  (0, 0),
  (140, 30),
  (150, 40),
  (0, 70),
  (10, 70),
  (20, 70),
  (140, 40),
  (90, 10),
  (100, 10),
  (110, 10),
  (136, 50),
  (30, 70),
  (40, 70),
  (50, 70),
];
