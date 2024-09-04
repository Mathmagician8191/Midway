// Server for WW2 naval combat simulator

use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::io::{stdin, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};

const PORT: u16 = 25565;

const TPS: u32 = 60;

#[derive(Clone)]
struct Ship {
  coords: (f32, f32),
  velocity: f32,
  angle: f32,
  helm: f32,
  power: f32,
}

impl Ship {
  fn new(coords: (f32, f32), angle: f32) -> Self {
    Self {
      coords,
      velocity: 0.0,
      angle,
      helm: 0.0,
      power: 0.0,
    }
  }

  fn step(&mut self) {
    self.angle += self.helm * 0.01;
    self.velocity = 0.997 * self.velocity + 0.004 * self.power;
    self.coords.0 += self.velocity * self.angle.sin();
    self.coords.1 -= self.velocity * self.angle.cos();
  }
}

enum ClientMessage {
  Sail(f32, f32),
}

struct ClientData {
  stream: TcpStream,
  rx: Receiver<ClientMessage>,
  ship: Ship,
}

impl ClientData {
  fn new(stream: TcpStream, rx: Receiver<ClientMessage>, ship: Ship) -> Self {
    Self { stream, rx, ship }
  }
}

fn process_joining(tx: Sender<(TcpStream, Receiver<ClientMessage>, String)>) {
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
      match words.next() {
        Some("ship") => match words.next() {
          Some(name) => name,
          None => {
            println!("Invalid input");
            continue;
          }
        },
        _ => {
          println!("Invalid input");
          continue;
        }
      }
    } else {
      println!("Invalid input");
      continue;
    };
    let (tx2, rx) = channel();
    spawn(|| process_client(stream, tx2));
    if tx.send((stream_clone, rx, name.to_owned())).is_ok() {
      println!("{address} connected as {name}");
    } else {
      // The game has started
      return;
    }
  }
}

fn process_client(mut stream: BufReader<TcpStream>, tx: Sender<ClientMessage>) -> Option<()> {
  let mut buf = String::new();
  while let Ok(chars) = stream.read_line(&mut buf) {
    if chars == 0 {
      None?
    }
    let mut words = buf.split_whitespace();
    match words.next() {
      Some("sail") => {
        let power = words.next().and_then(|w| w.parse().ok())?;
        let helm = words.next().and_then(|w| w.parse().ok())?;
        tx.send(ClientMessage::Sail(power, helm)).ok()?;
      }
      _ => todo!(),
    }
    buf.clear();
  }
  None
}

fn main() {
  let (tx, rx) = channel();
  spawn(|| process_joining(tx));
  let mut buf = String::new();
  stdin()
    .lock()
    .read_line(&mut buf)
    .expect("Failed to wait for input");
  println!("Starting game");
  let mut connections = HashMap::new();
  for (stream, rx, name) in rx.try_iter() {
    let address = stream
      .peer_addr()
      .map(|x| x.to_string())
      .unwrap_or("unknown".to_owned());
    println!("{address} joined as {name}");
    let mut rng = thread_rng();
    let x = rng.gen_range(-100.0..100.0);
    let y = rng.gen_range(-100.0..100.0);
    let ship = Ship::new((x, y), 0.0);
    let client = ClientData::new(stream, rx, ship);
    connections.insert(name, client);
  }
  let delay = Duration::from_secs(1) / TPS;
  loop {
    let start = Instant::now();
    for _ in 0..TPS {
      let start = Instant::now();
      let mut disconnected = Vec::new();
      // get updates from clients
      for (name, connection) in &mut connections {
        loop {
          match connection.rx.try_recv() {
            Ok(ClientMessage::Sail(power, helm)) => {
              connection.ship.power = power;
              connection.ship.helm = helm;
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
        for (_, connection) in &mut connections {
          connection
            .stream
            .write_all(format!("sunk {name}\n").as_bytes())
            .ok();
        }
      }
      let mut ships = Vec::new();
      for (name, connection) in &mut connections {
        connection.ship.step();
        ships.push((name.clone(), connection.ship.clone()));
      }
      for (name, ship) in ships {
        let (x, y) = ship.coords;
        let angle = ship.angle;
        for (_, connection2) in &mut connections {
          connection2
            .stream
            .write_all(format!("ship {name} {x} {y} {angle}\n").as_bytes())
            .ok();
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
