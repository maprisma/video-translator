// Maprisma Video Translator's caption window on COSMIC, the counterpart of the GNOME extension's
// captions.js and the Quickshell overlay on Omarchy. The panel applet keeps this helper running.
//
// It polls the caption controller's overlay-state on its owner-only socket (four times a second
// while captions are open, once a second otherwise; nothing is started while Maprisma Video Translator is
// off). When the controller opens the captions, a transparent layer surface covers the output on
// the overlay layer, above all windows and fullscreen video. Only the caption box takes input
// (the surface's input region), so clicks elsewhere go to the windows below.
//
// Drag the box with the left mouse button, resize it with the bottom-right handle (shown while
// the pointer is over the captions) and scroll it with the mouse wheel.
//
// Automatic caption colour: every 2 seconds, while the captions are open and Automatic is chosen,
// grim captures the 60-pixel strip just above the captions (below them at the top of the screen),
// scaled to 5 %, into memory; only its mean brightness goes to the controller. Nothing is stored.

use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use cosmic::app::{Core, Settings, Task};
use cosmic::iced::event::wayland::{Event as WaylandEvent, OutputEvent};
use cosmic::cctk::wayland_client::protocol::wl_output::WlOutput;
use cosmic::iced::event::{self, PlatformSpecific};
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{IcedOutput, SctkLayerSurfaceSettings};
use cosmic::iced::platform_specific::shell::wayland::commands::layer_surface::{
    Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface, set_input_zone,
};
use cosmic::iced::widget::{column as iced_column, container as iced_container, row as iced_row, scrollable, space};
use cosmic::iced::{Background, Border, Color, Event, Length, Limits, Point, Rectangle, Size, Subscription, mouse, window};
use cosmic::{Element, theme};
use serde_json::{Value, json};

// The applet's shared look.rs (the desktop's palette outside COSMIC) and the corners.rs it builds
// on; only the accent is used here (the caption theme's border).
#[allow(dead_code)]
#[path = "../../applet/src/corners.rs"]
mod corners;
#[allow(dead_code)]
#[path = "../../applet/src/look.rs"]
mod look;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

const POLL: Duration = Duration::from_millis(250);
const IDLE: Duration = Duration::from_millis(1000);
const SAMPLE: Duration = Duration::from_secs(2);
const FIRST_BORDER: Duration = Duration::from_millis(1500);
const BAND: f32 = 60.0;
const MIN_WIDTH: f32 = 430.0;
const GRIP: f32 = 28.0;
const SCROLLBAR: f32 = 12.0;
const MAX_REPLY: usize = 1024 * 1024;

fn main() -> cosmic::iced::Result {
    // Ends with the applet that started it, also when the applet is killed (a panel restart):
    // otherwise the old window would hold the lock and the new applet's copy would leave.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
    }
    if std::os::unix::process::parent_id() == 1 {
        return Ok(());
    }
    // One caption window per session: a second copy leaves at once.
    let Some(_lock) = single_instance() else {
        return Ok(());
    };
    let settings = Settings::default().no_main_window(true).exit_on_close(false).transparent(true);
    cosmic::app::run::<Captions>(settings, ())
}

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn single_instance() -> Option<std::fs::File> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(runtime_dir().join("maprisma-video-translator-captions.lock"))
        .ok()?;
    // SAFETY: flock on a file descriptor we own; the lock lives as long as the file.
    let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    locked.then_some(file)
}

// The controller's owner-only socket (bridge.socket_path() in the backend).
fn socket_path() -> PathBuf {
    // SAFETY: getuid cannot fail.
    let uid = unsafe { libc::getuid() };
    runtime_dir().join(format!("video-trans-{uid}.sock"))
}

// One request straight on the controller's socket; None when no controller runs.
async fn poll(payload: Value) -> Option<Value> {
    let path = socket_path();
    if !path.exists() {
        return None;
    }
    let ask = async {
        let mut stream = tokio::net::UnixStream::connect(path).await.ok()?;
        stream.write_all(format!("{payload}\n").as_bytes()).await.ok()?;
        let mut reader = BufReader::new(stream.take(MAX_REPLY as u64));
        let mut line = String::new();
        reader.read_line(&mut line).await.ok()?;
        let reply: Value = serde_json::from_str(&line).ok()?;
        (reply.is_object() && reply.get("error").is_none()).then_some(reply)
    };
    tokio::time::timeout(Duration::from_secs(2), ask).await.ok().flatten()
}

// Mean relative luminance (0..1) of a binary PPM (P6, maxval 255) from grim.
fn ppm_luminance(data: &[u8]) -> Option<f64> {
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 && i < data.len() {
        while i < data.len() && data[i].is_ascii_whitespace() {
            i += 1;
        }
        if data.get(i) == Some(&b'#') {
            while i < data.len() && data[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        while i < data.len() && !data[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(std::str::from_utf8(&data[start..i]).ok()?.to_string());
    }
    if fields.len() < 4 || fields[0] != "P6" || fields[3] != "255" {
        return None;
    }
    let (width, height): (usize, usize) = (fields[1].parse().ok()?, fields[2].parse().ok()?);
    let pixels = data.get(i + 1..i + 1 + width * height * 3)?;
    if pixels.is_empty() {
        return None;
    }
    let sum: f64 = pixels
        .chunks_exact(3)
        .map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64)
        .sum();
    Some(sum / (255.0 * (width * height) as f64))
}

// A failed capture disables sampling for this helper's lifetime. KWin has layer-shell
// but no wlr-screencopy; avoid even the first grim process there.
// (look::desktop: also without XDG_CURRENT_DESKTOP, from the session's other marks.)
fn sampling_available() -> bool {
    look::desktop() != look::Desktop::Kde
}
async fn sample(region: Rectangle) -> Option<f64> {
    if !sampling_available() {
        return None;
    }
    let geometry = format!(
        "{},{} {}x{}",
        region.x.round() as i32,
        region.y.round() as i32,
        region.width.round() as i32,
        region.height.round() as i32
    );
    let run = tokio::process::Command::new("grim")
        .args(["-g", &geometry, "-s", "0.05", "-t", "ppm", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(Duration::from_secs(3), run).await {
        Ok(Ok(out)) if out.status.success() => ppm_luminance(&out.stdout),
        // Tried again next time (a failure while outputs change passes);
        // desktops grim cannot capture at all never get here (see above).
        _ => None,
    }
}

fn rgb(hex: &str) -> Color {
    let value = hex.strip_prefix('#').filter(|h| h.len() == 6).and_then(|h| u32::from_str_radix(h, 16).ok());
    match value {
        Some(v) => Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8),
        None => Color::WHITE,
    }
}

fn hex(color: Color) -> String {
    let [r, g, b, _] = color.into_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn number(view: &Value, key: &str) -> Option<f64> {
    view.get(key).and_then(Value::as_f64).filter(|v| v.is_finite())
}

fn theme_color(view: &Value, key: &str) -> Color {
    if view.get("text_color").and_then(Value::as_str) == Some("auto") && !sampling_available() {
        match key {
            "foreground" => return Color::WHITE,
            "background" => return Color::BLACK,
            _ => {}
        }
    }
    rgb(view
        .get("theme")
        .and_then(|t| t.get(key))
        .and_then(Value::as_str)
        .unwrap_or(""))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Move,
    Resize,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    mode: Mode,
    from: Point,
    start: Rectangle,
}

#[derive(Debug, Clone)]
struct Output {
    output: WlOutput,
    id: u32,
    position: (i32, i32),
    size: (i32, i32),
}

#[derive(Default)]
struct Captions {
    core: Core,
    surface: Option<window::Id>,
    area: Option<Size>,  // the surface's size, once the compositor configured it
    rect: Option<Rectangle>, // the caption box inside the surface; None until placed
    view: Value,
    text: String,
    outputs: Vec<Output>,
    origin: (f32, f32), // the surface's output in global coordinates, for grim
    cursor: Option<Point>,
    drag: Option<Drag>,
    first_border_until: Option<Instant>,
    luminance: Option<f64>,
    polling: bool,
    sampling: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Poll,
    Polled(Option<Value>),
    Sample,
    Sampled(Option<f64>),
    Resized(window::Id, Size),
    Output(Output),
    OutputGone(u32),
    Moved(Point),
    Left,
    Pressed,
    Released,
    Tick,
}

static SCROLL_ID: std::sync::LazyLock<cosmic::widget::Id> =
    std::sync::LazyLock::new(|| cosmic::widget::Id::new("maprisma-video-translator-captions"));

impl Captions {
    fn open(&self) -> bool {
        self.view.get("open").and_then(Value::as_bool) == Some(true) && self.view.get("theme").is_some()
    }

    fn font_size(&self) -> f32 {
        number(&self.view, "font_size").unwrap_or(24.0).clamp(8.0, 96.0) as f32
    }

    // About three lines at the caption font size.
    fn min_height(&self) -> f32 {
        (self.font_size() * 96.0 / 72.0 * 1.25 * 3.0 + 28.0).ceil()
    }

    fn hover(&self) -> bool {
        matches!((self.cursor, self.rect), (Some(c), Some(r)) if r.contains(c))
    }

    // Centred near the bottom of the output, with the saved size.
    fn place(&mut self) -> Task<Message> {
        let (Some(area), Some(id)) = (self.area, self.surface) else {
            return Task::none();
        };
        let width = (number(&self.view, "width").unwrap_or(900.0) as f32).min(area.width).max(MIN_WIDTH.min(area.width));
        let height =
            (number(&self.view, "height").unwrap_or(460.0) as f32).min(area.height).max(self.min_height().min(area.height));
        self.rect = Some(Rectangle {
            x: ((area.width - width) / 2.0).max(0.0).round(),
            y: (area.height - height - 60.0).max(0.0).round(),
            width,
            height,
        });
        // The border shows briefly on opening, then only while the pointer is over the captions.
        self.first_border_until = Some(Instant::now() + FIRST_BORDER);
        self.input_zone(id)
    }

    fn input_zone(&self, id: window::Id) -> Task<Message> {
        let zone = self.rect.map(|r| vec![r]).unwrap_or_default();
        set_input_zone(id, Some(zone))
    }

    fn close(&mut self) -> Task<Message> {
        self.rect = None;
        self.area = None;
        self.drag = None;
        self.cursor = None;
        self.text.clear();
        match self.surface.take() {
            Some(id) => destroy_layer_surface(id),
            None => Task::none(),
        }
    }

    // On the first output the compositor announced (the only one on a single-monitor desktop):
    // its size and position are known at once, so the box is placed without waiting for the
    // compositor's configure. Without output information the active output is used.
    fn create(&mut self) -> Task<Message> {
        let id = window::Id::unique();
        self.surface = Some(id);
        let target = self.outputs.iter().min_by_key(|o| o.id).cloned();
        self.origin = target.as_ref().map(|o| (o.position.0 as f32, o.position.1 as f32)).unwrap_or((0.0, 0.0));
        self.area = target
            .as_ref()
            .filter(|o| o.size.0 > 0 && o.size.1 > 0)
            .map(|o| Size::new(o.size.0 as f32, o.size.1 as f32));
        let surface = get_layer_surface(SctkLayerSurfaceSettings {
            id,
            layer: Layer::Overlay,
            keyboard_interactivity: KeyboardInteractivity::None,
            // No input until the box is placed: clicks go to the windows below.
            input_zone: Some(Vec::new()),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            output: target.map(|o| IcedOutput::Output(o.output)).unwrap_or(IcedOutput::Active),
            namespace: "maprisma-video-translator-captions".into(),
            size: Some((None, None)),
            // The whole output, under the panel too, so box positions are output positions.
            exclusive_zone: -1,
            size_limits: Limits::NONE.min_width(1.0).min_height(1.0),
            ..Default::default()
        });
        surface.chain(self.place())
    }

    // The strip just above the captions (below them when they touch the top), in global coordinates.
    fn sample_region(&self) -> Option<Rectangle> {
        let (area, rect) = (self.area?, self.rect?);
        let top = if rect.y - BAND >= 0.0 { rect.y - BAND } else { rect.y + rect.height };
        let left = rect.x.max(0.0);
        let right = (rect.x + rect.width).min(area.width);
        let bottom = (top + BAND).min(area.height);
        if right <= left || bottom <= top {
            return None;
        }
        let (ox, oy) = self.origin;
        Some(Rectangle { x: ox + left, y: oy + top, width: right - left, height: bottom - top })
    }

    fn apply(&mut self, view: Option<Value>) -> Task<Message> {
        let Some(view) = view else {
            self.view = Value::Null;
            return self.close();
        };
        let was_open = self.surface.is_some();
        self.view = view;
        if !self.open() {
            return self.close();
        }
        let mut tasks = Vec::new();
        if !was_open {
            // The outputs arrive right after start; the surface waits for them (next poll).
            if self.outputs.is_empty() {
                self.view = Value::Null;
                return Task::none();
            }
            tasks.push(self.create());
        } else if self.rect.is_none() && self.area.is_some() {
            tasks.push(self.place());
        }
        let caption_received = self.view.get("caption_received").and_then(Value::as_bool) == Some(true);
        let mut text = self.view.get("text").and_then(Value::as_str).unwrap_or("").to_string();
        if text.is_empty() && !caption_received {
            text = Some(self.view.get("status").and_then(Value::as_str).unwrap_or(""))
                .filter(|s| !s.is_empty())
                .unwrap_or("Starting captions…")
                .to_string();
            text = maprisma_video_translator_control::tr(&text);
        }
        if text != self.text {
            self.text = text;
            // Follow new captions: stay at the bottom as the text grows.
            if self.view.get("auto_scroll").and_then(Value::as_bool) != Some(false) {
                tasks.push(scrollable::snap_to(
                    SCROLL_ID.clone(),
                    scrollable::RelativeOffset { x: None, y: Some(1.0) },
                ));
            }
        }
        // Too small for the font after a size change: grown to three lines.
        if let (Some(rect), Some(area), None) = (self.rect, self.area, self.drag) {
            let min = self.min_height().min(area.height);
            if rect.height < min {
                self.rect = Some(Rectangle { height: min, ..rect });
                if let Some(id) = self.surface {
                    tasks.push(self.input_zone(id));
                }
            }
        }
        Task::batch(tasks)
    }

    fn drag_to(&mut self, cursor: Point) -> Task<Message> {
        let (Some(drag), Some(area), Some(id)) = (self.drag, self.area, self.surface) else {
            return Task::none();
        };
        let (dx, dy) = (cursor.x - drag.from.x, cursor.y - drag.from.y);
        let start = drag.start;
        let rect = match drag.mode {
            Mode::Resize => Rectangle {
                width: (start.width + dx).min(area.width - start.x).max(MIN_WIDTH.min(area.width - start.x)).round(),
                height: (start.height + dy)
                    .min(area.height - start.y)
                    .max(self.min_height().min(area.height - start.y))
                    .round(),
                ..start
            },
            Mode::Move => Rectangle {
                x: (start.x + dx).min(area.width - 60.0).max(60.0 - start.width).round(),
                y: (start.y + dy).min(area.height - 60.0).max(0.0).round(),
                ..start
            },
        };
        self.rect = Some(rect);
        self.input_zone(id)
    }

    fn caption_box(&self) -> Element<'_, Message> {
        let view = &self.view;
        let foreground = theme_color(view, "foreground");
        let mut background = theme_color(view, "background");
        let transparency = number(view, "background_transparency").unwrap_or(100.0);
        background.a = (1.0 - transparency / 100.0).clamp(0.0, 1.0) as f32;
        let border_width = number(view, "border_width").unwrap_or(1.0).clamp(0.0, 16.0) as f32;
        let active = self.hover()
            || self.drag.is_some()
            || self.first_border_until.is_some_and(|until| Instant::now() < until);
        let border_color = if active { theme_color(view, "border_color") } else { Color::TRANSPARENT };

        let size = self.font_size() * 96.0 / 72.0;
        let text = cosmic::iced::widget::text(self.text.clone())
            .size(size)
            .line_height(cosmic::iced::widget::text::LineHeight::Relative(1.25))
            .class(foreground)
            .width(Length::Fill);
        let scroll = scrollable(iced_container(text).padding(14).width(Length::Fill))
            .id(SCROLL_ID.clone())
            .width(Length::Fill)
            .height(Length::Fill);

        let grip: Element<Message> = if self.hover() || self.drag.is_some_and(|d| d.mode == Mode::Resize) {
            iced_row![
                space::horizontal().width(Length::Fill),
                cosmic::iced::widget::text("◢").size(GRIP * 0.7).class(foreground)
            ]
            .height(Length::Fixed(GRIP))
            .into()
        } else {
            space::vertical().height(Length::Fixed(0.0)).into()
        };
        let rect = self.rect.unwrap_or_default();
        let inner: Element<Message> = cosmic::iced::widget::stack![
            scroll,
            iced_column![space::vertical().height(Length::Fill), grip].width(Length::Fill).height(Length::Fill),
        ]
        .into();
        iced_container(inner)
            .width(Length::Fixed(rect.width))
            .height(Length::Fixed(rect.height))
            .style(move |_| iced_container::Style {
                background: Some(Background::Color(background)),
                border: Border { color: border_color, width: border_width, radius: 12.0.into() },
                ..Default::default()
            })
            .into()
    }
}

impl cosmic::Application for Captions {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "com.maprisma.VideoTranslatorCaptions";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, _flags: ()) -> (Self, Task<Message>) {
        // The surface covers the whole output. libcosmic's automatic blur (frosted system
        // interface) would ask the compositor to blur everything behind it, i.e. the whole screen.
        core.set_auto_blur(Default::default());
        (Self { core, ..Default::default() }, Task::done(cosmic::Action::App(Message::Poll)))
    }

    fn subscription(&self) -> Subscription<Message> {
        let every = if self.surface.is_some() { POLL } else { IDLE };
        let mut subscriptions = vec![
            cosmic::iced::time::every(every).map(|_| Message::Poll),
            event::listen_with(|event, _status, id| match event {
                Event::Window(window::Event::Resized(size)) => Some(Message::Resized(id, size)),
                Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Message::Moved(position)),
                Event::Mouse(mouse::Event::CursorLeft) => Some(Message::Left),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => Some(Message::Pressed),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::Released),
                Event::PlatformSpecific(PlatformSpecific::Wayland(WaylandEvent::Output(event, output))) => {
                    use cosmic::cctk::wayland_client::Proxy;
                    let id = output.id().protocol_id();
                    match event {
                        OutputEvent::Created(Some(info)) | OutputEvent::InfoUpdate(info) => Some(Message::Output(Output {
                            output: output.clone(),
                            id,
                            position: info.logical_position.unwrap_or(info.location),
                            size: info.logical_size.unwrap_or_default(),
                        })),
                        OutputEvent::Removed => Some(Message::OutputGone(id)),
                        _ => None,
                    }
                }
                _ => None,
            }),
        ];
        if self.surface.is_some() {
            subscriptions.push(cosmic::iced::time::every(SAMPLE).map(|_| Message::Sample));
            // Redraws for the first-border timeout.
            if self.first_border_until.is_some() {
                subscriptions.push(cosmic::iced::time::every(Duration::from_millis(300)).map(|_| Message::Tick));
            }
        }
        Subscription::batch(subscriptions)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Poll => {
                if self.polling {
                    return Task::none();
                }
                self.polling = true;
                // The desktop's accent: COSMIC's theme, or (KDE, Hyprland) look.rs's palette,
                // re-read at most every 5 s.
                look::refresh();
                let accent = hex(look::theme().unwrap_or_else(theme::active).cosmic().accent_color().into());
                let mut payload = json!({
                    "action": "overlay-state",
                    "visible": self.surface.is_some(),
                    "accent": accent,
                });
                if let Some(luminance) = self.luminance.take() {
                    payload["luminance"] = json!(luminance);
                }
                return Task::perform(poll(payload), |view| cosmic::Action::App(Message::Polled(view)));
            }
            Message::Polled(view) => {
                self.polling = false;
                return self.apply(view);
            }
            Message::Resized(id, size) => {
                if self.surface == Some(id) && size.width > 1.0 && size.height > 1.0 {
                    let first = self.area.is_none();
                    self.area = Some(size);
                    if first || self.rect.is_none() {
                        return self.place();
                    }
                    // A changed output size: kept on screen.
                    if let Some(rect) = self.rect {
                        let width = rect.width.min(size.width);
                        let height = rect.height.min(size.height);
                        self.rect = Some(Rectangle {
                            x: rect.x.min(size.width - width).max(0.0),
                            y: rect.y.min(size.height - height).max(0.0),
                            width,
                            height,
                        });
                        return self.input_zone(id);
                    }
                }
            }
            Message::Output(output) => {
                self.outputs.retain(|o| o.id != output.id);
                self.outputs.push(output);
            }
            Message::OutputGone(id) => self.outputs.retain(|o| o.id != id),
            Message::Moved(position) => {
                self.cursor = Some(position);
                if self.drag.is_some() {
                    return self.drag_to(position);
                }
            }
            Message::Left => {
                if self.drag.is_none() {
                    self.cursor = None;
                }
            }
            Message::Pressed => {
                if let (Some(cursor), Some(rect)) = (self.cursor, self.rect) {
                    if rect.contains(cursor) {
                        let (x, y) = (cursor.x - rect.x, cursor.y - rect.y);
                        let resize = x > rect.width - GRIP - 4.0 && y > rect.height - GRIP - 4.0;
                        // The scrollbar stays the scrollable's.
                        if resize || x < rect.width - SCROLLBAR - 4.0 {
                            self.first_border_until = None;
                            self.drag = Some(Drag {
                                mode: if resize { Mode::Resize } else { Mode::Move },
                                from: cursor,
                                start: rect,
                            });
                        }
                    }
                }
            }
            Message::Released => {
                if let Some(drag) = self.drag.take() {
                    if drag.mode == Mode::Resize {
                        if let Some(rect) = self.rect {
                            let payload = json!({
                                "action": "overlay-geometry",
                                "width": rect.width.round() as i64,
                                "height": rect.height.round() as i64,
                            });
                            return Task::perform(poll(payload), |_| cosmic::Action::App(Message::Tick));
                        }
                    }
                }
            }
            Message::Sample => {
                let auto = self.view.get("text_color").and_then(Value::as_str) == Some("auto");
                if self.sampling || !auto || self.drag.is_some() || !sampling_available() {
                    return Task::none();
                }
                if let Some(region) = self.sample_region() {
                    self.sampling = true;
                    return Task::perform(sample(region), |l| cosmic::Action::App(Message::Sampled(l)));
                }
            }
            Message::Sampled(luminance) => {
                self.sampling = false;
                self.luminance = luminance;
            }
            Message::Tick => {
                if self.first_border_until.is_some_and(|until| Instant::now() >= until) {
                    self.first_border_until = None;
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        space::horizontal().width(Length::Fixed(0.0)).into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if self.surface != Some(id) || self.rect.is_none() {
            return space::horizontal().width(Length::Fill).into();
        }
        let rect = self.rect.unwrap_or_default();
        iced_column![
            space::vertical().height(Length::Fixed(rect.y)),
            iced_row![space::horizontal().width(Length::Fixed(rect.x)), self.caption_box()],
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: Color::WHITE,
            icon_color: Color::WHITE,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luminance_of_a_ppm() {
        let mut data = b"P6\n# grim\n2 1\n255\n".to_vec();
        data.extend_from_slice(&[255, 255, 255, 0, 0, 0]);
        let value = ppm_luminance(&data).unwrap();
        assert!((value - 0.5).abs() < 1e-9);
        assert!(ppm_luminance(b"P5\n1 1\n255\n\0").is_none());
        assert!(ppm_luminance(b"P6\n4 4\n255\n\0\0\0").is_none());
    }

    #[test]
    fn colours() {
        assert_eq!(hex(rgb("#3584e4")), "#3584e4");
        assert_eq!(rgb("nonsense"), Color::WHITE);
    }
}
