use crate::Ship;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::spawn;

const PORT: u16 = 25565;

pub enum ClientMessage {
  Sail(f32, f32),
  Anchor,
  Smoke,
  Weapon(u32),
}

pub struct ClientData {
  pub tx: Sender<String>,
  pub rx: Receiver<ClientMessage>,
  pub ship: Ship,
}

impl ClientData {
  pub fn new(mut stream: TcpStream, rx: Receiver<ClientMessage>, ship: Ship) -> Self {
    let (tx, rx_2) = channel::<String>();
    spawn(move || {
      for message in rx_2 {
        stream.write_all(message.as_bytes()).ok();
      }
    });
    Self { tx, rx, ship }
  }
}

pub fn process_joining(tx: &Sender<(TcpStream, Receiver<ClientMessage>, String)>) {
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
      Some("smoke") => tx.send(ClientMessage::Smoke).ok()?,
      Some("weapon") => {
        let weapon = words.next().and_then(|w| w.parse().ok())?;
        tx.send(ClientMessage::Weapon(weapon)).ok()?;
      }
      Some(word) => println!("Bad message {word} from client"),
      None => println!("Empty message from client"),
    }
    buf.clear();
  }
  None
}
