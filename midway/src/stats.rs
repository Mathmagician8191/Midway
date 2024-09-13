use crate::ShipStats;
use enum_iterator::{all, Sequence};
use random_pick::pick_from_slice;

#[derive(Clone, Copy, Sequence)]
enum ShipType {
  Escort,
  Destroyer,
  LightCruiser,
  HeavyCruiser,
  BattleCruiser,
  SlowBattleship,
  FastBattleship,
  Bird,
}

const WEIGHTS: &[usize] = &[15, 25, 4, 3, 1, 1, 1, 1];

fn get_random_type() -> ShipType {
  *pick_from_slice(&all::<ShipType>().collect::<Vec<ShipType>>(), WEIGHTS)
    .expect("Could not generate ship type")
}

const fn get_stats(ship: ShipType) -> ShipStats {
  match ship {
    ShipType::Escort => ShipStats {
      texture: 1,
      length: 93.3,
      mass: 1740.0,
      health: 1740.0,
      power: 5340.0,
      k: 0.043,
      surface_area: 608.4,
      froude_scale_factor: 2.2,
    },
    ShipType::Destroyer => ShipStats {
      texture: 2,
      length: 112.5,
      mass: 2050.0,
      health: 2050.0,
      power: 27000.0,
      k: 0.034,
      surface_area: 903.3,
      froude_scale_factor: 0.3,
    },
    ShipType::LightCruiser => ShipStats {
      texture: 3,
      length: 180.0,
      mass: 11932.0,
      health: 11932.0,
      power: 45000.0,
      k: 0.038,
      surface_area: 2301.0,
      froude_scale_factor: 2.3,
    },
    ShipType::HeavyCruiser => ShipStats {
      texture: 4,
      length: 176.0,
      mass: 12663.0,
      health: 12663.0,
      power: 47900.0,
      k: 0.035,
      surface_area: 1960.0,
      froude_scale_factor: 2.64,
    },
    ShipType::BattleCruiser => ShipStats {
      texture: 5,
      length: 228.7,
      mass: 27200.0,
      health: 27200.0,
      power: 50400.0,
      k: 0.044,
      surface_area: 3668.0,
      froude_scale_factor: 4.19,
    },
    ShipType::SlowBattleship => ShipStats {
      texture: 6,
      length: 190.27,
      mass: 33100.0,
      health: 33100.0,
      power: 13000.0,
      k: 0.074,
      surface_area: 3343.0,
      froude_scale_factor: 25.17,
    },
    ShipType::FastBattleship => ShipStats {
      texture: 6,
      length: 262.13,
      mass: 48880.0,
      health: 48800.0,
      power: 94800.0,
      k: 0.048,
      surface_area: 5257.0,
      froude_scale_factor: 5.63,
    },
    ShipType::Bird => ShipStats {
      texture: 7,
      length: 51.0,
      mass: 617.0,
      health: 617.0,
      power: 490.0,
      k: 0.097,
      surface_area: 336.4,
      froude_scale_factor: 14.48,
    },
  }
}

pub fn get_random_ship() -> ShipStats {
  get_stats(get_random_type())
}
