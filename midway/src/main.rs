// Server for WW2 naval combat simulator
use rand::{thread_rng, Rng};
use stats::get_random_ship;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};

mod stats;

const PORT: u16 = 25565;

const TIME_ACCELERATION_FACTOR: f32 = 5.0;
const TPS: u32 = 60;

const COLOUR: &str = "f00";

const MAP_RADIUS: Option<f32> = Some(5000.0);
const KRAKEN_NAME: &str = "Kraken";

const WATER_VISCOSITY: f32 = 0.000_001;
const GRAVITY: f32 = 9.81;

#[derive(Clone)]
struct Ship {
  coords: (f32, f32),
  velocity: f32,
  angle: f32,
  helm: f32,
  power: f32,
  stats: ShipStats,
  sunk: bool,
}

#[derive(Clone)]
struct ShipStats {
  texture: usize,
  length: f32,
  mass: f32,
  health: f32,
  power: f32,
  k: f32,
  surface_area: f32,
  froude_scale_factor: f32,
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
    }
  }

  fn step(&mut self, delta_t: f32) {
    self.angle += delta_t * self.helm * self.velocity / self.stats.length;
    let mut net_power = self.power * self.stats.power;
    let reynolds_number = self.stats.length * self.velocity.abs() / WATER_VISCOSITY;
    let c_f = 0.075 / (reynolds_number.log10() - 2.0).powi(2);
    let c_v = c_f * (1.0 + self.stats.k);
    let froude_number = self.velocity / (GRAVITY * self.stats.length).sqrt();
    let c_w = self.stats.froude_scale_factor * froude_number.powi(6);
    let c_total = c_v + c_w;
    let r_total = c_total * 0.5 * self.velocity.powi(2) * self.stats.surface_area;
    net_power -= r_total * self.velocity;
    let new_energy =
      0.5 * self.stats.mass * self.velocity * self.velocity.abs() + net_power * delta_t;
    self.velocity = match new_energy.total_cmp(&0.0) {
      Ordering::Greater => (2.0 * new_energy / self.stats.mass).sqrt(),
      Ordering::Less => -(-2.0 * new_energy / self.stats.mass).sqrt(),
      Ordering::Equal => 0.0,
    };
    self.coords.0 += self.velocity * delta_t * self.angle.sin();
    self.coords.1 -= self.velocity * delta_t * self.angle.cos();
  }

  fn distance_from_origin(&self) -> f32 {
    (self.coords.0.powi(2) + self.coords.1.powi(2)).sqrt()
  }

  fn distance(&self, other: &Self) -> f32 {
    let x_distance = self.coords.0 - other.coords.0;
    let y_distance = self.coords.1 - other.coords.1;
    (x_distance.powi(2) + y_distance.powi(2)).sqrt()
  }

  fn damage(&mut self, amount: f32) {
    self.stats.health -= amount;
    if self.stats.health <= 0.0 {
      self.sunk = true;
    }
  }
}

enum ClientMessage {
  Sail(f32, f32),
  Anchor,
}

struct ClientData {
  stream: TcpStream,
  rx: Receiver<ClientMessage>,
  ship: Ship,
}

impl ClientData {
  const fn new(stream: TcpStream, rx: Receiver<ClientMessage>, ship: Ship) -> Self {
    Self { stream, rx, ship }
  }
}

fn process_joining(tx: &Sender<(TcpStream, Receiver<ClientMessage>, String)>) {
  let listener = TcpListener::bind(format!("0.0.0.0:{PORT}"))
    .unwrap_or_else(|_| panic!("Failed to bind to port {PORT}"));

  for stream in listener.incoming().flatten() {
    let address = stream
      .peer_addr()
      .map(|x| x.to_string())
      .unwrap_or("unknown".to_owned());
    let stream_clone = stream.try_clone().expect("try-clone broke");
    let mut stream = BufReader::new(stream);
    let mut buf = String::new();
    let name = if let Ok(chars) = stream.read_line(&mut buf) {
      if chars == 0 {
        println!("{address} failed to connect");
        continue;
      }
      let mut words = buf.split_whitespace();
      if let Some("ship") = words.next() {
        if let Some(name) = words.next() {
          name
        } else {
          println!("Invalid input");
          continue;
        }
      } else {
        println!("Invalid input");
        continue;
      }
    } else {
      println!("Invalid input");
      continue;
    };
    let (tx2, rx) = channel();
    spawn(move || process_client(stream, &tx2));
    if tx.send((stream_clone, rx, name.to_owned())).is_ok() {
      println!("{address} connected as {name}");
    } else {
      // The server has crashed or something
      return;
    }
  }
}

fn process_client(mut stream: BufReader<TcpStream>, tx: &Sender<ClientMessage>) -> Option<()> {
  let mut buf = String::new();
  while let Ok(chars) = stream.read_line(&mut buf) {
    if chars == 0 {
      None?;
    }
    let mut words = buf.split_whitespace();
    match words.next() {
      Some("sail") => {
        let power = words.next().and_then(|w| w.parse().ok())?;
        let helm = words.next().and_then(|w| w.parse().ok())?;
        tx.send(ClientMessage::Sail(power, helm)).ok()?;
      }
      Some("anchor") => tx.send(ClientMessage::Anchor).ok()?,
      _ => todo!(),
    }
    buf.clear();
  }
  None
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
  if let Some(radius) = MAP_RADIUS {
    stream
      .write_all(format!("radius {radius}\n").as_bytes())
      .ok();
  }
  let client = ClientData::new(stream, rx, ship);
  connections.insert(name, client);
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
  loop {
    let start = Instant::now();
    for _ in 0..TPS {
      let start = Instant::now();
      // Process newly joining clients
      for (stream, rx, name) in rx.try_iter() {
        handle_join(&mut connections, stream, rx, name);
      }
      let mut disconnected = Vec::new();
      // get updates from clients
      for (name, connection) in &mut connections {
        loop {
          match connection.rx.try_recv() {
            Ok(ClientMessage::Sail(power, helm)) => {
              connection.ship.power = power;
              connection.ship.helm = helm;
            }
            Ok(ClientMessage::Anchor) => {
              if connection.ship.velocity < 1.5 {
                connection.ship.velocity = 0.0;
              }
            }
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
          connection
            .stream
            .write_all(format!("sunk {name}\n").as_bytes())
            .ok();
        }
      }
      let mut ships = Vec::new();
      let mut kraken_targets = 0;
      let mut names_to_remove = Vec::new();
      for (name, connection) in &mut connections {
        if connection.ship.sunk {
          continue;
        }
        if let Some(ref kraken) = kraken {
          if kraken.distance(&connection.ship) < 100.0 {
            connection.ship.velocity = 0.0;
            kraken_targets += 1;
            connection.ship.damage(20.0 * delta_t);
            if connection.ship.sunk {
              names_to_remove.push(name.clone());
              continue;
            }
          }
        }
        connection.ship.step(delta_t);
        if let Some(radius) = MAP_RADIUS {
          if kraken.is_none() && connection.ship.distance_from_origin() > radius {
            let mut rng = thread_rng();
            let angle = rng.gen_range(0.0..(2.0 * PI));
            let distance = rng.gen_range(40.0..80.0);
            let x = connection.ship.coords.0 + distance * angle.cos();
            let y = connection.ship.coords.1 + distance * angle.sin();
            let stats = ShipStats {
              texture: 8,
              length: 60.0,
              mass: 1000.0,
              health: 1000.0,
              power: 1000.0,
              k: 0.0,
              surface_area: 1000.0,
              froude_scale_factor: 2.2,
            };
            kraken = Some(Ship {
              coords: (x, y),
              velocity: 0.0,
              angle: 0.0,
              helm: 0.0,
              power: 0.0,
              stats,
              sunk: false,
            });
            connection.ship.velocity = 0.0;
            kraken_targets += 1;
          }
        }
        ships.push((name.clone(), connection.ship.clone()));
      }
      for name in names_to_remove {
        for connection in connections.values_mut() {
          connection
            .stream
            .write_all(format!("sunk {name}\n").as_bytes())
            .ok();
        }
      }
      if let Some(ref kraken_ship) = kraken {
        if kraken_targets == 0 || kraken_ship.sunk {
          kraken = None;
          let message = format!("sunk {KRAKEN_NAME}\n");
          for connection in connections.values_mut() {
            connection.stream.write_all(message.as_bytes()).ok();
          }
        }
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
        let health = ship.stats.health / ship.stats.mass;
        let message = format!("ship {name} {x} {y} {angle} {velocity} {size} {texture} {COLOUR} {health}\n");
        for connection2 in connections.values_mut() {
          connection2.stream.write_all(message.as_bytes()).ok();
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
