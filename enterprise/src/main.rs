use eframe::epaint::PathStroke;
use eframe::{egui, run_native, App, Frame, NativeOptions};
use egui::{
  include_image, pos2, vec2, Align2, CentralPanel, Color32, Context, FontId, Image, Key, Pos2,
  Rect, Rounding, Ui, Vec2, ViewportBuilder,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::spawn;

const SCREEN_SIZE: Pos2 = Pos2::new(1280.0, 800.0);
const SHIP_SIZE: f32 = 60.0;

#[derive(Default)]
struct ShipData {
  power: f32,
  helm: f32,
}

enum MidwayMessage {
  Ship(String, (Pos2, f32)),
  Sunk(String),
}

struct MidwayData {
  rx: Receiver<MidwayMessage>,
  stream: TcpStream,
  name: String,
  ship_data: ShipData,
  ships: HashMap<String, (Pos2, f32)>,
}

impl MidwayData {
  fn new(name: String, rx: Receiver<MidwayMessage>, stream: TcpStream) -> Self {
    Self {
      rx,
      stream,
      name,
      ship_data: ShipData::default(),
      ships: HashMap::new(),
    }
  }
}

enum Window {
  MainMenu(String, String, String, Option<&'static str>),
  Midway(MidwayData),
}

impl Default for Window {
  fn default() -> Self {
    Self::MainMenu(String::new(), String::new(), String::new(), None)
  }
}

#[derive(Default)]
struct Enterprise {
  window: Window,
}

impl App for Enterprise {
  fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
    CentralPanel::default().show(ctx, |ui| match &mut self.window {
      Window::MainMenu(name, ip, port, message) => {
        if let Some(stream) = draw_main_menu(ui, name, ip, port, message) {
          let (tx, rx) = channel();
          let stream_clone = stream.try_clone().expect("Try-clone broke");
          spawn(|| handle_midway_connection(stream_clone, tx));
          self.window = Window::Midway(MidwayData::new(name.to_string(), rx, stream));
        }
      }
      Window::Midway(ref mut data) => draw_midway(ui, data),
    });
    ctx.request_repaint();
  }
}

fn draw_main_menu(
  ui: &mut Ui,
  name: &mut String,
  ip: &mut String,
  port: &mut String,
  message: &mut Option<&'static str>,
) -> Option<TcpStream> {
  ui.label("Ship name");
  ui.text_edit_singleline(name);
  ui.label("Location of Midway");
  ui.text_edit_singleline(ip);
  ui.label("Port");
  ui.text_edit_singleline(port);
  if ui.button("Connect").clicked() {
    match format!("{ip}:{port}").parse::<SocketAddr>() {
      Ok(address) => match TcpStream::connect(address) {
        Ok(mut stream) => {
          if stream
            .write_all(format!("ship {name}\n").as_bytes())
            .is_ok()
          {
            return Some(stream);
          } else {
            *message = Some("Could not connect to Midway");
          }
        }
        Err(_) => *message = Some("Could not connect to Midway"),
      },
      Err(_) => *message = Some("Invalid ip address"),
    }
  }
  if let Some(message) = message {
    ui.label(*message);
  }
  None
}

fn draw_midway(ui: &mut Ui, data: &mut MidwayData) {
  ui.ctx().input(|i| {
    data.ship_data.helm = match (i.key_down(Key::A), i.key_down(Key::D)) {
      (true, false) => -1.0,
      (false, true) => 1.0,
      (true, true) | (false, false) => 0.0,
    };
    match (i.key_down(Key::Z), i.key_down(Key::X), i.key_down(Key::C)) {
      (true, false, false) => data.ship_data.power = 1.0,
      (false, true, false) => data.ship_data.power = 0.0,
      (false, false, true) => data.ship_data.power = -0.5,
      (false, false, false) => {
        if i.key_down(Key::W) {
          data.ship_data.power += 0.01;
        }
        if i.key_down(Key::S) {
          data.ship_data.power -= 0.01;
        }
        data.ship_data.power = data.ship_data.power.clamp(-0.5, 1.0);
      }
      _ => (),
    };
  });
  data
    .stream
    .write_all(format!("sail {} {}\n", data.ship_data.power, data.ship_data.helm).as_bytes())
    .ok();
  for message in data.rx.try_iter() {
    match message {
      MidwayMessage::Ship(name, position) => data.ships.insert(name.to_string(), position),
      MidwayMessage::Sunk(name) => data.ships.remove(&name),
    };
  }
  let painter = ui.painter();
  let offset = match data.ships.get(&data.name) {
    Some((ship, _)) => SCREEN_SIZE / 2.0 - *ship,
    None => Vec2::ZERO,
  };
  let x_offset_modulo = offset.x % 100.0;
  for x in 0..=12_i16 {
    let x = f32::from(x) * 100.0 + x_offset_modulo;
    painter.line_segment(
      [pos2(x, 0.0), pos2(x, SCREEN_SIZE.y)],
      PathStroke::new(2.0, Color32::BLUE),
    );
  }
  let y_offset_modulo = offset.y % 100.0;
  for y in 0..=8_i16 {
    let y = f32::from(y) * 100.0 + y_offset_modulo;
    painter.line_segment(
      [pos2(0.0, y), pos2(SCREEN_SIZE.x, y)],
      PathStroke::new(2.0, Color32::BLUE),
    );
  }
  for (ship, (coords, angle)) in &data.ships {
    let coords = *coords + offset;
    painter.text(
      coords - vec2(0.0, SHIP_SIZE / 2.0),
      Align2::CENTER_CENTER,
      ship,
      FontId::default(),
      Color32::WHITE,
    );
    let rect = Rect::from_center_size(coords, Vec2::splat(SHIP_SIZE));
    Image::new(include_image!("../../resources/TestShip.png"))
      .tint(Color32::GRAY)
      .rotate(*angle, Vec2::splat(0.5))
      .paint_at(ui, rect);
  }
  painter.line_segment(
    [pos2(0.0, 650.0), pos2(20.0, 650.0)],
    PathStroke::new(1.0, Color32::WHITE),
  );
  painter.line_segment(
    [pos2(0.0, 750.0), pos2(20.0, 750.0)],
    PathStroke::new(1.0, Color32::WHITE),
  );
  match data.ship_data.power.total_cmp(&0.0) {
    Ordering::Greater => {
      let rect = Rect {
        min: pos2(0.0, 750.0 - 100.0 * data.ship_data.power),
        max: pos2(20.0, 750.0),
      };
      painter.rect_filled(rect, Rounding::ZERO, Color32::GREEN);
    }
    Ordering::Less => {
      let rect = Rect {
        min: pos2(0.0, 750.0),
        max: pos2(20.0, 750.0 - 100.0 * data.ship_data.power),
      };
      painter.rect_filled(rect, Rounding::ZERO, Color32::RED);
    }
    Ordering::Equal => (),
  }
}

fn handle_midway_connection(stream: TcpStream, tx: Sender<MidwayMessage>) -> Option<()> {
  let mut stream = BufReader::new(stream);
  let mut buf = String::new();
  while let Ok(chars) = stream.read_line(&mut buf) {
    if chars == 0 {
      None?
    }
    let mut words = buf.split_whitespace();
    match words.next() {
      Some("ship") => {
        let name = match words.next() {
          Some(name) => name,
          None => {
            println!("Invalid input");
            buf.clear();
            continue;
          }
        };
        let x = match words.next().and_then(|w| w.parse().ok()) {
          Some(x) => x,
          None => {
            println!("Invalid input");
            buf.clear();
            continue;
          }
        };
        let y = match words.next().and_then(|w| w.parse().ok()) {
          Some(x) => x,
          None => {
            println!("Invalid input");
            buf.clear();
            continue;
          }
        };
        let coords = pos2(x, y);
        let angle = match words.next().and_then(|w| w.parse().ok()) {
          Some(x) => x,
          None => {
            println!("Invalid input");
            buf.clear();
            continue;
          }
        };
        tx.send(MidwayMessage::Ship(name.to_string(), (coords, angle)))
          .ok()?;
      }
      Some("sunk") => {
        let name = match words.next() {
          Some(name) => name,
          None => {
            println!("Invalid input");
            buf.clear();
            continue;
          }
        };
        tx.send(MidwayMessage::Sunk(name.to_string())).ok()?;
      }
      _ => println!("Unknown line"),
    }
    buf.clear();
  }
  None
}

fn main() {
  let viewport = ViewportBuilder::default()
    .with_min_inner_size(SCREEN_SIZE.to_vec2())
    .with_max_inner_size(SCREEN_SIZE.to_vec2())
    .with_resizable(false);
  let options = NativeOptions {
    viewport,
    ..Default::default()
  };
  run_native(
    "Enterprise",
    options,
    Box::new(|cc| {
      egui_extras::install_image_loaders(&cc.egui_ctx);
      Ok(Box::new(Enterprise::default()))
    }),
  )
  .expect("Failed to reach Midway");
}
