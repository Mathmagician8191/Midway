//! Server for WW2 naval combat simulator
use client::{process_joining, ClientData, ClientMessage};
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use stats::{get_random_ship, ShipStats};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};

mod client;
mod stats;

const TIME_ACCELERATION_FACTOR: f32 = 4.0;
const TPS: u32 = 60;
const RESPAWN_COOLDOWN: u32 = 120;

const COLOUR: &str = "999";

const MAP_RADIUS: Option<(f32, BorderType)> = Some((
  2000.0,
  BorderType::Ocean(OceanData {
    kraken_spawn_chance: 0.01,
    mine_spawn_chance: 0.0001,
    mine_damage: 2000.0,
    scale: 500.0,
    intensity: 40.0,
    dps: 1.0,
  }),
));
const KRAKEN_NAME: &str = "Kraken";

const WATER_VISCOSITY: f32 = 0.000_001;
const GRAVITY: f32 = 9.81;

#[allow(unused)]
enum BorderType {
  Ocean(OceanData),
  Land,
}

struct OceanData {
  kraken_spawn_chance: f32,
  mine_spawn_chance: f32,
  mine_damage: f32,
  scale: f32,
  intensity: f32,
  dps: f32,
}

// Works on negative numbers too
fn cube_root(x: f32) -> f32 {
  let result = x.abs().powf(1.0 / 3.0);
  result.copysign(x)
}

#[derive(Clone)]
struct Ship {
  coords: (f32, f32),
  velocity: f32,
  angle: f32,
  helm: f32,
  power: f32,
  stats: ShipStats,
  sunk: bool,
  smoke: bool,
  respawn_cooldown: u32,
}

impl Ship {
  fn new() -> Self {
    let mut rng = thread_rng();
    let angle = rng.gen_range(0.0..(2.0 * PI));
    let distance = rng.gen_range(0.0..1000.0);
    let x = distance * angle.cos();
    let y = distance * angle.sin();
    let stats = get_random_ship();
    Self {
      coords: (x, y),
      velocity: 0.0,
      angle: 0.0,
      helm: 0.0,
      power: 0.0,
      stats,
      sunk: false,
      smoke: false,
      respawn_cooldown: RESPAWN_COOLDOWN,
    }
  }

  fn step(&mut self, delta_t: f32) {
    self.stats.cooldown -= delta_t;
    self.angle += delta_t * self.helm * self.velocity * 2.0 / self.stats.turning_circle;
    let reynolds_number = self.stats.length * self.velocity.abs() / WATER_VISCOSITY;
    let c_f = 0.075 / (reynolds_number.log10() - 2.0).powi(2);
    let c_v = c_f * (1.0 + self.stats.k);
    let froude_number = self.velocity / (GRAVITY * self.stats.length).sqrt();
    let c_w = self.stats.froude_scale_factor * froude_number.powi(6);
    let c_total = c_v + c_w;
    let r_total = c_total * 0.5 * self.velocity * self.velocity.abs() * self.stats.surface_area;
    let net_power = self.power * self.stats.power;
    let q = net_power / self.stats.screw_area;
    let s = q.powi(2) - self.velocity.powi(6) / 27.0;
    let v_out = match s.total_cmp(&0.0) {
      Ordering::Equal => 2.0 * cube_root(q),
      Ordering::Greater => {
        let s = s.sqrt();
        cube_root(q + s) + cube_root(q - s)
      }
      Ordering::Less => {
        let v_abs = self.velocity.abs();
        let multiplier = (2.0 * v_abs / 3.0_f32.sqrt()).copysign(q);
        let angle = (q.abs() * 27.0_f32.sqrt() / v_abs.powi(3)).acos();
        multiplier * (angle / 3.0).cos()
      }
    };
    let thrust = self.stats.screw_area * v_out * (v_out - self.velocity).abs();
    let net_thrust = thrust - r_total;
    self.velocity += net_thrust * delta_t / self.stats.mass;
    self.coords.0 += self.velocity * delta_t * self.angle.sin();
    self.coords.1 -= self.velocity * delta_t * self.angle.cos();
  }

  fn energy(&self) -> f32 {
    0.5 * self.stats.mass * self.velocity.powi(2)
  }

  fn distance_from_origin(&self) -> f32 {
    self.coords.0.hypot(self.coords.1)
  }

  fn distance(&self, other: &Self) -> f32 {
    let x_distance = self.coords.0 - other.coords.0;
    let y_distance = self.coords.1 - other.coords.1;
    x_distance.hypot(y_distance)
  }

  #[must_use]
  fn damage(&mut self, amount: f32) -> bool {
    self.stats.health -= amount;
    if self.stats.health <= 0.0 {
      self.sunk = true;
    }
    self.sunk
  }

  #[must_use]
  fn shoot(&mut self, target: &mut Self) -> ShootingState {
    if self.stats.cooldown <= 0.0 {
      let mut rng = thread_rng();
      let damage = self.stats.gun_damage * rng.gen_range(0.5..1.5);
      self.stats.cooldown = rng.gen_range(self.stats.gun_reload_time.clone());
      if target.damage(damage) {
        ShootingState::Sunk(damage)
      } else {
        ShootingState::Hit(damage)
      }
    } else {
      ShootingState::NotFired
    }
  }
}

enum ShootingState {
  NotFired,
  Hit(f32),
  Sunk(f32),
}

fn handle_join(
  connections: &mut HashMap<String, ClientData>,
  mut stream: TcpStream,
  rx: Receiver<ClientMessage>,
  name: String,
) {
  let address = stream
    .peer_addr()
    .map(|x| x.to_string())
    .unwrap_or("unknown".to_owned());
  println!("{address} joined as {name}");
  let ship = Ship::new();
  if let Some((radius, ..)) = MAP_RADIUS {
    stream
      .write_all(format!("radius {radius}\n").as_bytes())
      .ok();
  }
  let client = ClientData::new(stream, rx, ship);
  connections.entry(name).or_insert(client);
}

fn main() {
  let (tx, rx) = channel();
  spawn(move || process_joining(&tx));
  let mut connections = HashMap::new();
  let (stream, rx_2, name) = rx.recv().expect("Could not start server");
  handle_join(&mut connections, stream, rx_2, name);
  let delay = Duration::from_secs(1) / TPS;
  let delta_t = TIME_ACCELERATION_FACTOR / TPS as f32;
  let mut kraken: Option<Ship> = None;
  let mut kraken_cooldown = 0.0;
  loop {
    let start = Instant::now();
    for _ in 0..TPS {
      kraken_cooldown -= delta_t;
      let start = Instant::now();
      // Process newly joining clients
      for (stream, rx, name) in rx.try_iter() {
        handle_join(&mut connections, stream, rx, name);
      }
      let mut disconnected = Vec::new();
      let mut splashes = Vec::new();
      // get updates from clients
      for (name, connection) in &mut connections {
        loop {
          match connection.rx.try_recv() {
            Ok(ClientMessage::Sail(power, helm)) => {
              connection.ship.power = power * power.abs();
              connection.ship.helm = helm;
            }
            Ok(ClientMessage::Anchor) => {
              if connection.ship.velocity.abs() < 0.5 {
                connection.ship.velocity = 0.0;
              }
            }
            Ok(ClientMessage::Smoke) => {
              connection.ship.smoke = !connection.ship.smoke;
            }
            Ok(ClientMessage::Weapon(_)) => (),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
              println!("{name} has disconnected");
              disconnected.push(name.clone());
              break;
            }
          }
        }
      }
      for name in &disconnected {
        connections.remove(name);
      }
      for name in disconnected {
        for connection in connections.values_mut() {
          connection.tx.send(format!("sunk {name}\n")).ok();
        }
      }
      let mut kraken_targets = Vec::new();
      let mut sunk = Vec::new();
      for (name, connection) in &mut connections {
        let ship = &mut connection.ship;
        if ship.sunk {
          if ship.respawn_cooldown == 0 {
            *ship = Ship::new();
          } else {
            ship.respawn_cooldown -= 1;
          }
          continue;
        }
        let mut mobile = true;
        if let Some(ref mut kraken) = kraken {
          let distance = kraken.distance(ship);
          if distance < kraken.stats.gun_range {
            ship.velocity = 0.0;
            mobile = false;
            kraken_targets.push(name.clone());
          }
          if distance < ship.stats.gun_range {
            match ship.shoot(kraken) {
              ShootingState::Sunk(damage) | ShootingState::Hit(damage) => {
                let mut rng = thread_rng();
                let max_offset = kraken.stats.length / 2.0;
                let min_offset = -max_offset;
                let splash_x = kraken.coords.0 + rng.gen_range(min_offset..max_offset);
                let splash_y = kraken.coords.1 + rng.gen_range(min_offset..max_offset);
                let size = damage.powf(1.0 / 3.0) * 3.0;
                splashes.push((splash_x, splash_y, size, 1.0, 0, "f00"));
                let max_offset = ship.stats.length / 2.0;
                let min_offset = -max_offset;
                let location = rng.gen_range(min_offset..max_offset);
                let splash_x = ship.coords.0 + location * ship.angle.sin();
                let splash_y = ship.coords.1 - location * ship.angle.cos();
                splashes.push((splash_x, splash_y, size, 1.0, 1, "fff"));
              }
              ShootingState::NotFired => (),
            }
          }
        }
        let mut rng = thread_rng();
        if ship.smoke && rng.gen_bool(f64::from(delta_t * ship.power.abs())) {
          splashes.push((
            ship.coords.0,
            ship.coords.1,
            (ship.power.abs() * ship.stats.power).sqrt() * rng.gen_range(0.5..1.5),
            rng.gen_range(30.0..180.0),
            2,
            "0009",
          ));
        }
        ship.step(delta_t);
        if let Some((radius, border)) = MAP_RADIUS {
          let ship_distance = ship.distance_from_origin();
          if ship_distance > radius {
            match border {
              BorderType::Ocean(data) => {
                let scale_factor = (ship_distance - radius) / (ship_distance - radius + data.scale);
                if ship.damage(data.dps * scale_factor * delta_t) {
                  sunk.push(name.clone());
                  continue;
                } else if mobile {
                  let scale_factor = data.intensity * scale_factor / ship_distance;
                  ship.coords.0 -= ship.coords.0 * scale_factor * delta_t;
                  ship.coords.1 -= ship.coords.1 * scale_factor * delta_t;
                }
                let mut mine_chance =
                  data.mine_spawn_chance * ship.velocity.abs() * ship.stats.beam * delta_t;
                if mine_chance > 1.0 {
                  mine_chance = 1.0;
                }
                if rng.gen_bool(f64::from(mine_chance)) {
                  let max_offset = ship.stats.length / 2.0;
                  let min_offset = -max_offset;
                  let location = rng.gen_range(min_offset..max_offset);
                  let splash_x = ship.coords.0 + location * ship.angle.sin();
                  let splash_y = ship.coords.1 - location * ship.angle.cos();
                  let damage = data.mine_damage * rng.gen_range(0.2..1.0);
                  splashes.push((
                    splash_x,
                    splash_y,
                    damage.powf(1.0 / 3.0) * 3.0,
                    1.0,
                    0,
                    "f00",
                  ));
                  if ship.damage(damage) {
                    sunk.push(name.clone());
                  }
                  ship.velocity *= ship.stats.mass / (ship.stats.mass + damage);
                }
                if kraken.is_none()
                  && kraken_cooldown <= 0.0
                  && rng.gen_bool(f64::from(data.kraken_spawn_chance * delta_t))
                {
                  let scale_factor = (ship_distance - radius) / data.scale + 1.0;
                  let scale_factor_sqrt = scale_factor.sqrt();
                  let angle = rng.gen_range(0.0..(2.0 * PI));
                  let distance = rng.gen_range(40.0..80.0) * scale_factor_sqrt;
                  let x = ship.coords.0 + distance * angle.cos();
                  let y = ship.coords.1 + distance * angle.sin();
                  let size = 60.0 * scale_factor_sqrt;
                  let stats = ShipStats::new(
                    8,
                    size,
                    size,
                    3000.0 * scale_factor,
                    0.0,
                    0.0,
                    1000.0,
                    0.0,
                    0.0,
                    2.2,
                    100.0 * scale_factor_sqrt,
                    100.0 * scale_factor_sqrt,
                    0.5..1.5,
                  );
                  let kraken_ship = Ship {
                    coords: (x, y),
                    velocity: 0.0,
                    angle: 0.0,
                    helm: 0.0,
                    power: 0.0,
                    stats,
                    sunk: false,
                    smoke: false,
                    respawn_cooldown: RESPAWN_COOLDOWN,
                  };
                  if kraken_ship.distance_from_origin() > radius {
                    kraken = Some(kraken_ship);
                    ship.velocity = 0.0;
                    kraken_targets.push(name.clone());
                  }
                }
              }
              BorderType::Land => {
                let energy = ship.energy();
                if ship.damage(energy / 1000.0) {
                  sunk.push(name.clone());
                }
                ship.velocity = 0.0;
                ship.stats.power = 0.0;
              }
            }
          }
        }
      }
      for name in sunk {
        let message = format!("sunk {name}\n");
        for connection in connections.values_mut() {
          connection.tx.send(message.clone()).ok();
        }
      }
      for (x, y, size, duration, sprite, colour) in splashes {
        let duration = duration / TIME_ACCELERATION_FACTOR;
        let message = format!("splash {x} {y} {size} {duration} {sprite} #{colour}\n");
        for connection in connections.values_mut() {
          connection.tx.send(message.clone()).ok();
        }
      }
      if let Some(ref mut kraken_ship) = kraken {
        kraken_ship.stats.cooldown -= delta_t;
        if kraken_ship.sunk {
          kraken_cooldown = kraken_ship.stats.mass / 50.0 + 60.0;
          kraken = None;
          let message = format!("sunk {KRAKEN_NAME}\n");
          for connection in connections.values_mut() {
            connection.tx.send(message.clone()).ok();
          }
        } else if let Some(target) = kraken_targets.choose(&mut thread_rng()) {
          let target_ship = &mut connections.get_mut(target).expect("Missing target").ship;
          match kraken_ship.shoot(target_ship) {
            ShootingState::Sunk(_) => {
              let message = format!("sunk {target}\n");
              for connection in connections.values_mut() {
                connection.tx.send(message.clone()).ok();
              }
            }
            ShootingState::Hit(_) | ShootingState::NotFired => (),
          }
        } else {
          kraken_cooldown = (kraken_ship.stats.mass - kraken_ship.stats.health) / 100.0 + 30.0;
          kraken = None;
          let message = format!("sunk {KRAKEN_NAME}\n");
          for connection in connections.values_mut() {
            connection.tx.send(message.clone()).ok();
          }
        }
      }
      let mut ships = Vec::new();
      for (name, connection) in &mut connections {
        ships.push((name.clone(), connection.ship.clone()));
      }
      if let Some(ref kraken) = kraken {
        ships.push((KRAKEN_NAME.to_string(), kraken.clone()));
      }
      for (name, ship) in ships {
        let (x, y) = ship.coords;
        let angle = ship.angle;
        let velocity = ship.velocity;
        let texture = ship.stats.texture;
        let size = ship.stats.length;
        let mut health = ship.stats.health / ship.stats.mass;
        if health < 0.0 {
          health = 0.0;
        }
        let message =
          format!("ship {name} {x} {y} {angle} {velocity} {size} {texture} #{COLOUR} {health}\n");
        for connection2 in connections.values_mut() {
          connection2.tx.send(message.clone()).ok();
        }
      }
      if connections.is_empty() {
        return;
      }
      let elapsed = start.elapsed();
      if delay > elapsed {
        sleep(delay - elapsed);
      }
    }
    let extra = (start.elapsed() - Duration::from_secs(1)).as_millis();
    if extra > 100 {
      println!("Can't keep up, is the server overloaded? {extra} ms behind");
    }
  }
}
