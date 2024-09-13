use eframe::epaint::PathStroke;
use eframe::{egui, run_native, App, Frame, NativeOptions};
use egui::{
  include_image, pos2, vec2, Align2, CentralPanel, Color32, Context, FontId, Image, ImageSource,
  Key, Pos2, Rect, Rounding, Ui, Vec2, ViewportBuilder,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::spawn;

const LONG_DEGREE_INTERVAL: f32 = 40_000_000.0 / 360.0;
const LAT_DEGREE_INTERVAL: f32 = 10_000_000.0 / 180.0;
const LONG_MINUTE_INTERVAL: f32 = LONG_DEGREE_INTERVAL / 60.0;
const LAT_MINUTE_INTERVAL: f32 = LAT_DEGREE_INTERVAL / 60.0;
const LINE_INTERVAL: f32 = LAT_MINUTE_INTERVAL;

const TEXTURES: &[ImageSource] = &[
  include_image!("../../resources/Missing.png"),
  include_image!("../../resources/DestroyerEscort.png"),
  include_image!("../../resources/Destroyer.png"),
  include_image!("../../resources/LightCruiser.png"),
  include_image!("../../resources/HeavyCruiser.png"),
  include_image!("../../resources/BattleCruiser.png"),
  include_image!("../../resources/Battleship.png"),
  include_image!("../../resources/LazerKiwi.png"),
  include_image!("../../resources/Kraken.png"),
];

struct Ship {
  coords: Pos2,
  angle: f32,
  velocity: f32,
  texture: usize,
  colour: Color32,
  size: f32,
  health: f32,
}

#[derive(Default)]
struct ShipData {
  power: f32,
  helm: f32,
}

enum MidwayMessage {
  Ship(String, Ship),
  Sunk(String),
  Radius(f32),
}

struct MidwayData {
  rx: Receiver<MidwayMessage>,
  stream: TcpStream,
  name: String,
  scale: i32,
  radius: Option<f32>,
  ship_data: ShipData,
  ships: HashMap<String, Ship>,
}

impl MidwayData {
  fn new(name: String, rx: Receiver<MidwayMessage>, stream: TcpStream) -> Self {
    Self {
      rx,
      stream,
      name,
      scale: 0,
      radius: None,
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

struct RenderState {
  scale: f32,
  offset: Pos2,
}

impl RenderState {
  fn new(scale: f32, center: Pos2, screen_center: Pos2) -> Self {
    Self {
      scale,
      offset: screen_center - center.to_vec2() * scale,
    }
  }

  fn scale(&self, size: f32) -> f32 {
    size * self.scale
  }

  fn transform(&self, position: Pos2) -> Pos2 {
    position * self.scale + self.offset.to_vec2()
  }

  // Screen space to real world
  fn reverse_transform(&self, position: Pos2) -> Pos2 {
    (position - self.offset.to_vec2()) / self.scale
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
          spawn(move || handle_midway_connection(stream_clone, &tx));
          self.window = Window::Midway(MidwayData::new(name.clone(), rx, stream));
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

fn draw_midway(ui: &Ui, data: &mut MidwayData) {
  let screen_size = ui.clip_rect().right_bottom();
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
    if i.key_down(Key::V) {
      data.stream.write_all(b"anchor\n").ok();
    }
    if (data.scale < 25) && i.key_pressed(Key::Minus) {
      data.scale += 1;
    }
    if (data.scale > -5) && i.key_pressed(Key::Equals) {
      data.scale -= 1;
    }
  });
  data
    .stream
    .write_all(format!("sail {} {}\n", data.ship_data.power, data.ship_data.helm).as_bytes())
    .ok();
  for message in data.rx.try_iter() {
    match message {
      MidwayMessage::Ship(name, position) => {
        data.ships.insert(name.to_string(), position);
      }
      MidwayMessage::Sunk(name) => {
        data.ships.remove(&name);
      }
      MidwayMessage::Radius(radius) => data.radius = Some(radius),
    };
  }
  let painter = ui.painter();
  let ship_coords = match data.ships.get(&data.name) {
    Some(ship) => ship.coords,
    None => Pos2::ZERO,
  };
  let scale = 0.9_f32.powi(data.scale);
  let render_state = RenderState::new(scale, ship_coords, screen_size / 2.0);
  let top_left = render_state.reverse_transform(Pos2::ZERO);
  let bottom_right = render_state.reverse_transform(screen_size);
  // Show the map
  if let Some(radius) = data.radius {
    let center = render_state.transform(Pos2::ZERO);
    let radius = render_state.scale(radius);
    painter.circle_filled(center, radius, Color32::DARK_BLUE);
  }
  // High quality ocean texture
  let Vec2 {
    x: delta_x,
    y: delta_y,
  } = bottom_right - top_left;
  let x_wave_count = (delta_x / LINE_INTERVAL) as i32;
  let y_wave_count = (delta_y / LINE_INTERVAL) as i32;
  let x_offset_modulo = f32::ceil(top_left.x / LINE_INTERVAL) * LINE_INTERVAL;
  for x in 0..=x_wave_count {
    let x = (x as f32).mul_add(LINE_INTERVAL, x_offset_modulo);
    let Pos2 { x, y: _ } = render_state.transform(pos2(x, 0.0));
    painter.vline(x, 0.0..=screen_size.y, PathStroke::new(2.0, Color32::BLUE));
  }
  let y_offset_modulo = f32::ceil(top_left.y / LINE_INTERVAL) * LINE_INTERVAL;
  for y in 0..=y_wave_count {
    let y = (y as f32).mul_add(LINE_INTERVAL, y_offset_modulo);
    let Pos2 { x: _, y } = render_state.transform(pos2(0.0, y));
    painter.hline(0.0..=screen_size.x, y, PathStroke::new(2.0, Color32::BLUE));
  }
  // Ships
  for (ship, data) in &data.ships {
    let coords = render_state.transform(data.coords);
    let scale = render_state.scale(data.size);
    painter.text(
      coords - vec2(0.0, scale / 2.0),
      Align2::CENTER_BOTTOM,
      ship,
      FontId::proportional(3.0 * scale.sqrt()),
      data.colour,
    );
    let rect = Rect::from_center_size(coords, Vec2::splat(scale));
    Image::new(TEXTURES[data.texture].clone())
      .tint(data.colour)
      .rotate(data.angle, Vec2::splat(0.5))
      .paint_at(ui, rect);
    if data.health < 1.0 {
      let height = scale.sqrt();
      let width = 10.0 * height;
      let baseline = coords + vec2(-width / 2.0, scale / 2.0);
      let current = Rect {
        min: baseline,
        max: baseline + vec2(data.health * width, height),
      };
      painter.rect_filled(current, Rounding::ZERO, Color32::GREEN);
      let lost = Rect {
        min: baseline + vec2(data.health * width, 0.0),
        max: baseline + vec2(width, height),
      };
      painter.rect_filled(lost, Rounding::ZERO, Color32::RED);
    }
  }
  // Location
  let latitude = match ship_coords.y.total_cmp(&0.0) {
    Ordering::Greater => {
      let degrees = (ship_coords.y / LAT_DEGREE_INTERVAL) as i16;
      let remainder = ship_coords.y % LAT_DEGREE_INTERVAL;
      let minutes = (remainder / LAT_MINUTE_INTERVAL) as i16;
      format!("{degrees}° {minutes}' S")
    }
    Ordering::Less => {
      let degrees = (-ship_coords.y / LAT_DEGREE_INTERVAL) as i16;
      let remainder = -ship_coords.y % LAT_DEGREE_INTERVAL;
      let minutes = (remainder / LAT_MINUTE_INTERVAL) as i16;
      format!("{degrees}° {minutes}' N")
    }
    Ordering::Equal => "0°".to_string(),
  };
  let longitude = match ship_coords.x.total_cmp(&0.0) {
    Ordering::Greater => {
      let degrees = (ship_coords.x / LONG_DEGREE_INTERVAL) as i16;
      let remainder = ship_coords.x % LONG_DEGREE_INTERVAL;
      let minutes = (remainder / LONG_MINUTE_INTERVAL) as i16;
      format!("{degrees}° {minutes}' E")
    }
    Ordering::Less => {
      let degrees = (-ship_coords.x / LONG_DEGREE_INTERVAL) as i16;
      let remainder = -ship_coords.x % LONG_DEGREE_INTERVAL;
      let minutes = (remainder / LONG_MINUTE_INTERVAL) as i16;
      format!("{degrees}° {minutes}' W")
    }
    Ordering::Equal => "0°".to_string(),
  };
  painter.text(
    Pos2::ZERO,
    Align2::LEFT_TOP,
    format!("{latitude} {longitude}"),
    FontId::proportional(20.0),
    Color32::WHITE,
  );
  if let Some(ship) = data.ships.get(&data.name) {
    // Speed
    painter.text(
      pos2(0.0, screen_size.y - 160.0),
      Align2::LEFT_BOTTOM,
      format!("{:.2} kt", ship.velocity * 2.0),
      FontId::proportional(20.0),
      Color32::WHITE,
    );
    // Throttle
    let top_throttle = screen_size.y - 150.0;
    painter.line_segment(
      [pos2(0.0, top_throttle), pos2(20.0, top_throttle)],
      PathStroke::new(1.0, Color32::WHITE),
    );
    let mid_throttle = screen_size.y - 50.0;
    painter.line_segment(
      [pos2(0.0, mid_throttle), pos2(20.0, mid_throttle)],
      PathStroke::new(1.0, Color32::WHITE),
    );
    match data.ship_data.power.total_cmp(&0.0) {
      Ordering::Greater => {
        let rect = Rect {
          min: pos2(0.0, mid_throttle - 100.0 * data.ship_data.power),
          max: pos2(20.0, mid_throttle),
        };
        painter.rect_filled(rect, Rounding::ZERO, Color32::GREEN);
      }
      Ordering::Less => {
        let rect = Rect {
          min: pos2(0.0, mid_throttle),
          max: pos2(20.0, mid_throttle - 100.0 * data.ship_data.power),
        };
        painter.rect_filled(rect, Rounding::ZERO, Color32::RED);
      }
      Ordering::Equal => (),
    }
  }
}

fn handle_midway_connection(stream: TcpStream, tx: &Sender<MidwayMessage>) -> Option<()> {
  let mut stream = BufReader::new(stream);
  let mut buf = String::new();
  while let Ok(chars) = stream.read_line(&mut buf) {
    if chars == 0 {
      None?;
    }
    let mut words = buf.split_whitespace();
    match words.next() {
      Some("ship") => {
        let Some(name) = words.next() else {
          println!("Invalid input");
          buf.clear();
          continue;
        };
        let Some(x) = words.next().and_then(|w| w.parse().ok()) else {
          println!("Invalid input");
          buf.clear();
          continue;
        };
        let Some(y) = words.next().and_then(|w| w.parse().ok()) else {
          println!("Invalid input");
          buf.clear();
          continue;
        };
        let coords = pos2(x, y);
        let Some(angle) = words.next().and_then(|w| w.parse().ok()) else {
          println!("Invalid input");
          buf.clear();
          continue;
        };
        let Some(velocity) = words.next().and_then(|w| w.parse().ok()) else {
          println!("Invalid input");
          buf.clear();
          continue;
        };
        let size = words.next().and_then(|w| w.parse().ok()).unwrap_or(60.0);
        let mut texture = words.next().and_then(|w| w.parse().ok()).unwrap_or(0);
        if texture >= TEXTURES.len() {
          texture = 0;
        }
        let colour = words
          .next()
          .and_then(|w| Color32::from_hex(w).ok())
          .unwrap_or(Color32::GRAY);
        let Some(health) = words.next().and_then(|w| w.parse().ok()) else {
          println!("Invalid input");
          buf.clear();
          continue;
        };
        let ship = Ship {
          coords,
          angle,
          velocity,
          texture,
          colour,
          size,
          health,
        };
        tx.send(MidwayMessage::Ship(name.to_string(), ship)).ok()?;
      }
      Some("sunk") => {
        let Some(name) = words.next() else {
          println!("Invalid input");
          buf.clear();
          continue;
        };
        tx.send(MidwayMessage::Sunk(name.to_string())).ok()?;
      }
      Some("radius") => {
        let Some(radius) = words.next().and_then(|w| w.parse().ok()) else {
          println!("Invalid input");
          buf.clear();
          continue;
        };
        tx.send(MidwayMessage::Radius(radius)).ok()?;
      }
      _ => println!("Unknown line"),
    }
    buf.clear();
  }
  None
}

fn main() {
  let viewport = ViewportBuilder::default()
    .with_min_inner_size(vec2(480.0, 360.0))
    .with_inner_size(vec2(1280.0, 800.0))
    .with_resizable(true);
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
