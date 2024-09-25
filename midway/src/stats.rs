use enum_iterator::{all, Sequence};
use random_pick::pick_from_slice;
use std::ops::Range;

const WEIGHTS: &[usize] = &[15, 25, 4, 3, 1, 1, 1, 1, 10, 10];

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
  PTBoat,
  Liberty,
}

#[derive(Clone)]
pub struct ShipStats {
  pub texture: usize,
  pub length: f32,
  pub beam: f32,
  pub mass: f32,
  pub health: f32,
  pub power: f32,
  pub k: f32,
  pub surface_area: f32,
  pub screw_area: f32,
  pub froude_scale_factor: f32,
  pub turning_circle: f32,
  pub gun_damage: f32,
  pub gun_range: f32,
  pub gun_reload_time: Range<f32>,
  pub cooldown: f32,
}

impl ShipStats {
  pub const fn new(
    texture: usize,
    length: f32,
    beam: f32,
    mass: f32,
    power: f32,
    k: f32,
    surface_area: f32,
    screw_area: f32,
    turning_circle: f32,
    froude_scale_factor: f32,
    gun_damage: f32,
    gun_range: f32,
    gun_reload_time: Range<f32>,
  ) -> Self {
    Self {
      texture,
      length,
      beam,
      mass,
      health: mass,
      power,
      k,
      surface_area,
      screw_area,
      froude_scale_factor,
      turning_circle,
      gun_damage,
      gun_range,
      gun_reload_time,
      cooldown: 0.0,
    }
  }
}

fn get_random_type() -> ShipType {
  *pick_from_slice(&all::<ShipType>().collect::<Vec<ShipType>>(), WEIGHTS)
    .expect("Could not generate ship type")
}

const fn get_stats(ship: ShipType) -> ShipStats {
  match ship {
    ShipType::Escort => ShipStats::new(
      1,
      93.3,
      11.1,
      1740.0,
      5933.0,
      0.066,
      608.4,
      4.54,
      560.0, // TODO: acquire proper value
      1.97,
      27.0,
      13400.0,
      0.4..0.44,
    ),
    ShipType::Destroyer => ShipStats::new(
      2,
      112.5,
      12.0,
      2500.0,
      30000.0,
      0.0263,
      903.3,
      11.45, // Warning - based off AI generated answer
      560.0,
      0.295,
      125.0,
      16000.0,
      0.8..1.2,
    ),
    ShipType::LightCruiser => ShipStats::new(
      3,
      180.0,
      20.22,
      14358.0,
      50000.0,
      0.062,
      2301.0,
      46.57,
      660.0,
      2.34,
      216.0,
      18288.0,
      0.5..0.625,
    ),
    ShipType::HeavyCruiser => ShipStats::new(
      4,
      176.0,
      18.82,
      12663.0,
      53200.0,
      0.091,
      1960.0,
      27.53,
      660.0,
      2.52,
      512.0,
      27480.0,
      1.33..2.0,
    ),
    ShipType::BattleCruiser => ShipStats::new(
      5,
      228.7,
      27.5,
      27636.0,
      56000.0,
      0.079,
      3668.0,
      52.81,
      860.0,
      4.2,
      3375.0,
      30680.0,
      4.0..6.0,
    ),
    ShipType::SlowBattleship => ShipStats::new(
      6,
      190.27,
      29.67,
      33100.0,
      14400.0,
      0.184,
      3343.0,
      67.93,
      640.0,
      25.57,
      4096.0,
      31364.0,
      4.0..6.0,
    ),
    ShipType::FastBattleship => ShipStats::new(
      6,
      262.13,
      32.97,
      48880.0,
      105333.0,
      0.107,
      5257.0,
      87.94,
      920.0,
      5.63,
      4096.0,
      38700.0,
      2.6..4.0,
    ),
    ShipType::Bird => ShipStats::new(
      7,
      51.0,
      9.1,
      938.0,
      547.0,
      0.112,
      336.4,
      4.337, // Estimate based on draft
      500.0, // TODO: acquire proper value
      13.8,
      64.0,
      12660.0,
      5.0..6.0,
    ),
    ShipType::PTBoat => ShipStats::new(
      9,
      24.0,
      6.3,
      57.0,
      2267.0,
      0.163,
      80.13,
      0.6744, // Estimate based on draft
      395.0,  // Note: value from earlier model of PT boat
      0.00067,
      4.096,
      7160.0,
      0.6..0.75,
    ),
    ShipType::Liberty => ShipStats::new(
      10,
      134.57,
      17.3,
      14474.0,
      1267.0,
      0.168,
      1638.6,
      14.186, // Estimate based on draft
      750.0,  // TODO: acquire proper value
      330.6,
      64.0,
      12660.0,
      5.0..6.0,
    ),
  }
}

pub fn get_random_ship() -> ShipStats {
  get_stats(get_random_type())
}
