// Maprisma Video Translator: the subtitles icon in the panel. Left click opens the menu
// (Start / Stop / Captions, what needs setting up, target language, input, caption size and the
// Maprisma Video Translator window); right click starts or stops captions. A green line under the icon
// while captions run.
//
// The caption controller (backend/gui.py, hidden) is reached through backend/bridge.sh like the
// GNOME extension did; the applet also keeps the caption window helper
// (maprisma-video-translator-captions) running, which draws the captions while the controller shows them.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::applet::cosmic_panel_config::PanelAnchor;
use cosmic::applet::{menu_button, padded_control};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::destroy_popup;
use cosmic::iced::widget::{container as iced_container, text as iced_text};
use cosmic::iced::{Alignment, Background, Border, Color, Length, Rectangle, Subscription, stream, window};
use cosmic::widget::rectangle_tracker::{RectangleTracker, RectangleUpdate, rectangle_tracker_subscription};
use cosmic::widget::{
    Id, autosize, button, column, container, divider, icon, mouse_area, row, scrollable, slider, space, text, toggler,
};
use cosmic::{Element, theme};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

const APP_ID: &str = "io.github.ubruckhaus.CosmicMaprismaVideoTranslator";
const ICON: &str = "media-view-subtitles-symbolic";
const POPUP_WIDTH: f32 = 360.0;
const FONT: (f32, f32) = (12.0, 64.0);
const RESTART_DELAY: Duration = Duration::from_secs(5);
static AUTOSIZE_ID: LazyLock<Id> = LazyLock::new(|| Id::new("maprisma-video-translator-autosize"));

const GREEN: Color = Color::from_rgb(0x33 as f32 / 255.0, 0xd1 as f32 / 255.0, 0x7a as f32 / 255.0);

const LANGUAGES: [&str; 18] = [
    "German", "English", "French", "Spanish", "Italian", "Portuguese", "Dutch", "Polish", "Ukrainian", "Russian",
    "Japanese", "Chinese", "Korean", "Arabic", "Hindi", "Turkish", "Indonesian", "Vietnamese",
];
const INPUTS: [(&str, &str); 2] =
    [("Audio output", "Audio output · what you hear (video, calls)"), ("Microphone", "Microphone · speech in the room")];

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<Translator>(())
}

// ------------------------------------------------------------------------- paths

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
}

// VIDEO_TRANSLATOR_BACKEND (development) or the installed copy.
fn bridge_script() -> PathBuf {
    std::env::var_os("VIDEO_TRANSLATOR_BACKEND")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_home().join("maprisma-video-translator/backend"))
        .join("bridge.sh")
}

// The controller's owner-only socket (bridge.socket_path() in the backend).
fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    // SAFETY: getuid cannot fail.
    let uid = unsafe { libc::getuid() };
    runtime.join(format!("video-trans-{uid}.sock"))
}

fn sibling(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| name.into())
}

fn spawn(program: PathBuf) {
    let _ = std::process::Command::new(program)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

// ------------------------------------------------------------------------- controller

// One request through the bridge (one JSON line in, one out): the controller's state with the
// setup steps, or {error}. The bridge starts the controller only for actions that need it.
async fn request(payload: Value) -> Value {
    let run = async {
        let mut child = tokio::process::Command::new("bash")
            .arg(bridge_script())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| e.to_string())?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(format!("{payload}\n").as_bytes()).await.map_err(|e| e.to_string())?;
        }
        let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        serde_json::from_str::<Value>(text.trim().lines().last().unwrap_or("{}")).map_err(|e| e.to_string())
    };
    match tokio::time::timeout(Duration::from_secs(60), run).await {
        Ok(Ok(state)) => state,
        Ok(Err(error)) => json!({ "error": format!("Maprisma Video Translator: {error}") }),
        Err(_) => json!({ "error": "Maprisma Video Translator: the caption controller did not answer." }),
    }
}

// The light poll ({running, starting}) straight on the controller's socket, so nothing is
// started while Maprisma Video Translator is off. None when no controller runs.
async fn brief() -> Option<Value> {
    let path = socket_path();
    if !path.exists() {
        return None;
    }
    let ask = async {
        let mut stream = tokio::net::UnixStream::connect(path).await.ok()?;
        stream.write_all(b"{\"action\": \"brief\"}\n").await.ok()?;
        let mut line = String::new();
        BufReader::new(stream.take(65536)).read_line(&mut line).await.ok()?;
        serde_json::from_str::<Value>(&line).ok().filter(|v| v.get("error").is_none())
    };
    tokio::time::timeout(Duration::from_secs(2), ask).await.ok().flatten()
}

fn flag(state: &Value, key: &str) -> bool {
    state.get(key).and_then(Value::as_bool) == Some(true)
}

fn string<'a>(state: &'a Value, key: &str) -> &'a str {
    state.get(key).and_then(Value::as_str).unwrap_or("")
}

fn setting<'a>(state: &'a Value, key: &str) -> Option<&'a Value> {
    state.get("settings").and_then(|s| s.get(key))
}

#[derive(Debug, Clone)]
struct Fix {
    label: String,
    requests: Vec<Value>,
    panel: bool,
}

// The requests a setup fix stands for ({label, action?, settings?, panel?} from readiness.steps()).
fn fix_requests(fix: &Value) -> Vec<Value> {
    let mut requests = Vec::new();
    if let Some(settings) = fix.get("settings").filter(|s| s.is_object()) {
        requests.push(json!({"action": "configure", "settings": settings}));
    }
    if let Some(action) = fix.get("action").and_then(Value::as_str) {
        requests.push(json!({ "action": action }));
    }
    if fix.get("settings").and_then(|s| s.get("audio_input")).is_some() {
        requests.push(json!({"action": "outputs"}));
    }
    requests
}

// What stands in the way of captions: the running setup, or the first failed setup step with
// the buttons that fix it. None when everything is ready.
fn next_step(state: &Value) -> Option<(String, Vec<Fix>)> {
    let setup = state.get("setup").unwrap_or(&Value::Null);
    if flag(setup, "busy") {
        let message = Some(string(setup, "message")).filter(|m| !m.is_empty()).unwrap_or("Setting up…");
        return Some((message.into(), Vec::new()));
    }
    let failed = state
        .get("steps")
        .and_then(Value::as_array)?
        .iter()
        .find(|step| step.get("ok").and_then(Value::as_bool) != Some(true))?;
    let mut problem = string(failed, "problem").to_string();
    if string(setup, "phase") == "error" && string(failed, "id") == "runtime" {
        problem = format!("{} {problem}", string(setup, "message"));
    }
    let fixes = failed
        .get("fixes")
        .and_then(Value::as_array)
        .map(|fixes| {
            fixes
                .iter()
                .map(|fix| Fix {
                    label: string(fix, "label").into(),
                    requests: fix_requests(fix),
                    // A fix with a panel ("provider") also opens the Maprisma Video Translator window.
                    panel: fix.get("panel").is_some_and(|p| !p.is_null() && p != &Value::Bool(false)),
                })
                .collect()
        })
        .unwrap_or_default();
    Some((problem, fixes))
}

fn notify(text: &str) {
    let _ = std::process::Command::new("notify-send")
        .args(["--app-name=Maprisma Video Translator", "-i", ICON, "Maprisma Video Translator", text])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

// ------------------------------------------------------------------------- widgets

fn dim_text(theme: &cosmic::Theme) -> iced_text::Style {
    let mut color = Color::from(theme.cosmic().on_bg_color());
    color.a = 0.6;
    iced_text::Style { color: Some(color), ..Default::default() }
}

fn fill(color: Color, radius: f32) -> theme::Container<'static> {
    theme::Container::custom(move |_| iced_container::Style {
        background: Some(Background::Color(color)),
        border: Border { radius: radius.into(), ..Default::default() },
        ..Default::default()
    })
}

// ------------------------------------------------------------------------- applet

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Target,
    Input,
}

#[derive(Default)]
struct Translator {
    core: Core,
    popup: Option<window::Id>,
    rectangle: Rectangle,
    tracker: Option<RectangleTracker<u32>>,
    state: Value, // the full state, while the menu is open
    running: bool,
    font: Option<f32>, // while the slider is being moved
    busy: usize,       // requests on their way
    expanded: Option<Section>,
}

#[derive(Debug, Clone)]
enum Message {
    Poll,
    Brief(Option<Value>),
    State(Value),
    Answer(Value),
    Send(Value),
    Fix(usize),
    Expand(Section),
    TogglePopup,
    CloseRequested(window::Id),
    Rectangle(RectangleUpdate<u32>),
    Font(f32),
    FontDone,
    OpenWindow,
    Released,
}

impl Translator {
    fn settings_font(&self) -> f32 {
        setting(&self.state, "font_size").and_then(Value::as_f64).unwrap_or(24.0).clamp(12.0, 64.0) as f32
    }

    fn panel_content(&self) -> Element<'_, Message> {
        let size = self.core.applet.suggested_size(true).0;
        let subtitles = icon::from_name(ICON).size(size).symbolic(true).icon();
        let line: Element<Message> = if self.running {
            container(space::horizontal())
                .width(Length::Fixed(size as f32))
                .height(Length::Fixed(2.0))
                .class(fill(GREEN, 1.0))
                .into()
        } else {
            space::vertical().height(Length::Fixed(2.0)).into()
        };
        column![subtitles, line].spacing(2).align_x(Alignment::Center).into()
    }

    fn menu(&self) -> Element<'_, Message> {
        let state = &self.state;
        let loaded = state.get("settings").is_some();
        let steps: Vec<&Value> = state.get("steps").and_then(Value::as_array).map(|s| s.iter().collect()).unwrap_or_default();
        let step_ok = |step: &&Value| step.get("ok").and_then(Value::as_bool) == Some(true);
        let runtime_ready = loaded && !steps.iter().any(|s| string(s, "id") == "runtime" && !step_ok(s));
        let ready = loaded && !steps.is_empty() && steps.iter().all(step_ok);
        let setup = state.get("setup").unwrap_or(&Value::Null);
        let (starting, downloading, stopping) = (flag(state, "starting"), flag(state, "downloading"), flag(state, "stopping"));
        let locked = self.running || starting || downloading || stopping;
        let idle = self.busy == 0;

        // Status, with the setup message and failed startup checks.
        let setup_message = if flag(setup, "busy") || string(setup, "phase") == "error" { string(setup, "message") } else { "" };
        let mut status = vec![
            Some(setup_message)
                .filter(|m| !m.is_empty())
                .or(Some(string(state, "status")).filter(|s| !s.is_empty()))
                .unwrap_or("Maprisma Video Translator")
                .to_string(),
        ];
        if let Some(checks) = state.get("checks").and_then(Value::as_array) {
            for check in checks.iter().filter(|c| string(c, "status") == "error") {
                status.push(format!("✕ {}: {}", string(check, "label"), string(check, "detail")));
            }
        }
        let header = row![icon::from_name(ICON).size(24).symbolic(true).icon(), text::body(status.join("\n")).width(Length::Fill)]
            .spacing(10)
            .align_y(Alignment::Center);

        let control = |icon_name: &'static str, label: &'static str, action: &'static str, enabled: bool| {
            button::standard(label)
                .leading_icon(icon::from_name(icon_name).size(16).symbolic(true))
                .width(Length::Fill)
                .on_press_maybe((enabled && idle).then(|| Message::Send(json!({ "action": action }))))
        };
        let controls = row![
            control(
                "media-playback-start-symbolic",
                "Start",
                "start",
                ready && !self.running && !stopping && !downloading && !flag(setup, "busy")
            ),
            control("media-playback-stop-symbolic", "Stop", "stop", (self.running || flag(state, "overlay")) && !stopping),
            control(ICON, "Captions", "show", runtime_ready),
        ]
        .spacing(6);

        let mut top = column![header, controls].spacing(10);
        if let Some((problem, fixes)) = loaded.then(|| next_step(state)).flatten() {
            let mut block = column![text::caption(problem).class(theme::Text::Custom(dim_text))].spacing(6);
            for (index, fix) in fixes.iter().enumerate() {
                let face = if index == 0 { button::suggested(fix.label.clone()) } else { button::standard(fix.label.clone()) };
                block = block.push(face.width(Length::Fill).on_press_maybe(idle.then_some(Message::Fix(index))));
            }
            top = top.push(block);
        }
        if downloading {
            top = top.push(
                button::standard("Cancel download")
                    .width(Length::Fill)
                    .on_press(Message::Send(json!({"action": "cancel-download"}))),
            );
        }

        let mut actions = column![divider::horizontal::default()];
        if loaded {
            let target = setting(state, "target").and_then(Value::as_str).unwrap_or("German");
            let input = setting(state, "audio_input").and_then(Value::as_str).unwrap_or("Audio output");
            actions = actions.push(self.expander(Section::Target, format!("Translate into · {target}"), !locked));
            if self.expanded == Some(Section::Target) && !locked {
                let list = LANGUAGES.iter().fold(column![], |list, language| {
                    list.push(self.choice(
                        language,
                        *language == target,
                        json!({"action": "configure", "settings": {"target": language}}),
                    ))
                });
                actions = actions.push(scrollable(list).height(Length::Fixed(240.0)));
            }
            actions = actions.push(self.expander(Section::Input, format!("Listen to · {input}"), runtime_ready && !locked));
            if self.expanded == Some(Section::Input) && runtime_ready && !locked {
                for (value, label) in INPUTS {
                    actions = actions.push(self.choice(
                        label,
                        value == input,
                        json!({"action": "configure", "settings": {"audio_input": value, "output": ""}}),
                    ));
                }
            }

            let follow = setting(state, "auto_scroll").and_then(Value::as_bool) != Some(false);
            actions = actions.push(padded_control(
                row![
                    text::body("Follow new captions").width(Length::Fill),
                    toggler(follow).on_toggle(|on| Message::Send(json!({"action": "configure", "settings": {"auto_scroll": on}}))),
                ]
                .align_y(Alignment::Center),
            ));
            let font = self.font.unwrap_or_else(|| self.settings_font());
            actions = actions.push(padded_control(
                row![
                    icon::from_name("preferences-desktop-font-symbolic").size(16).symbolic(true),
                    slider(FONT.0..=FONT.1, font, Message::Font).step(1.0_f32).on_release(Message::FontDone),
                    text::body(format!("{font:.0} pt")).width(Length::Fixed(48.0)),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ));
            actions = actions.push(
                menu_button(text::body("Clear captions"))
                    .on_press_maybe(runtime_ready.then(|| Message::Send(json!({"action": "clear"})))),
            );
        }
        actions = actions
            .push(divider::horizontal::default())
            .push(menu_button(text::body("Open Maprisma Video Translator…")).on_press(Message::OpenWindow));

        column![padded_control(top).padding([12, 16]), actions]
            .width(Length::Fixed(POPUP_WIDTH))
            .padding([0, 0, 8, 0])
            .into()
    }

    fn expander(&self, section: Section, label: String, enabled: bool) -> Element<'_, Message> {
        let open = self.expanded == Some(section) && enabled;
        let arrow = if open { "go-up-symbolic" } else { "go-down-symbolic" };
        menu_button(
            row![text::body(label).width(Length::Fill), icon::from_name(arrow).size(16).symbolic(true)]
                .align_y(Alignment::Center),
        )
        .on_press_maybe(enabled.then_some(Message::Expand(section)))
        .into()
    }

    fn choice(&self, label: &str, selected: bool, payload: Value) -> Element<'_, Message> {
        let mark: Element<Message> = if selected {
            icon::from_name("object-select-symbolic").size(16).symbolic(true).into()
        } else {
            space::horizontal().width(Length::Fixed(16.0)).into()
        };
        menu_button(row![mark, text::body(label.to_string())].spacing(8).padding([0, 0, 0, 12]).align_y(Alignment::Center))
            .on_press_maybe((!selected).then_some(Message::Send(payload)))
            .into()
    }

    fn close_popup(&mut self) -> Task<Message> {
        self.expanded = None;
        match self.popup.take() {
            Some(popup) => Task::batch([destroy_popup(popup), self.release()]),
            None => Task::none(),
        }
    }

    // Like closing the Omarchy popup: a controller without captions on screen exits.
    fn release(&self) -> Task<Message> {
        Task::perform(request(json!({"action": "release"})), |_| cosmic::Action::App(Message::Released))
    }

    fn send(&mut self, payload: Value) -> Task<Message> {
        self.busy += 1;
        Task::perform(request(payload), |state| cosmic::Action::App(Message::Answer(state)))
    }

    fn open_window(&mut self) -> Task<Message> {
        spawn(sibling("maprisma-video-translator"));
        self.close_popup()
    }
}

impl cosmic::Application for Translator {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: ()) -> (Self, Task<Message>) {
        (Self { core, ..Default::default() }, Task::done(cosmic::Action::App(Message::Poll)))
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::CloseRequested(id))
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            rectangle_tracker_subscription(0).map(|update| Message::Rectangle(update.1)),
            cosmic::iced::time::every(Duration::from_millis(1500)).map(|_| Message::Poll),
            Subscription::run_with("maprisma-video-translator-captions", |_| {
                stream::channel(1, |_output: cosmic::iced::futures::channel::mpsc::Sender<Message>| async move {
                    run_captions().await;
                })
            }),
        ])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Light poll for the icon; while the menu is open the full state instead.
            Message::Poll => {
                if self.popup.is_some() {
                    if self.busy == 0 {
                        return Task::perform(request(json!({"action": "state"})), |s| {
                            cosmic::Action::App(Message::State(s))
                        });
                    }
                } else {
                    return Task::perform(brief(), |s| cosmic::Action::App(Message::Brief(s)));
                }
            }
            Message::Brief(state) => self.running = state.is_some_and(|s| flag(&s, "running")),
            Message::State(state) => {
                if state.get("error").is_none() && state.get("settings").is_some() && self.busy == 0 {
                    self.running = flag(&state, "running");
                    self.state = state;
                }
            }
            Message::Answer(state) => {
                self.busy = self.busy.saturating_sub(1);
                if let Some(error) = state.get("error").and_then(Value::as_str) {
                    notify(error);
                } else if state.get("settings").is_some() {
                    self.running = flag(&state, "running");
                    self.state = state;
                }
            }
            Message::Send(payload) => {
                let settings = payload.get("settings");
                if settings.is_some_and(|s| s.get("target").is_some() || s.get("audio_input").is_some()) {
                    self.expanded = None;
                }
                let input = settings.and_then(|s| s.get("audio_input")).is_some();
                let task = self.send(payload);
                if input {
                    let outputs = self.send(json!({"action": "outputs"}));
                    return task.chain(outputs);
                }
                return task;
            }
            Message::Fix(index) => {
                if let Some((_, fixes)) = next_step(&self.state) {
                    if let Some(fix) = fixes.get(index).cloned() {
                        let mut tasks: Vec<Task<Message>> = fix.requests.into_iter().map(|r| self.send(r)).collect();
                        if fix.panel {
                            tasks.push(self.open_window());
                        }
                        return Task::batch(tasks);
                    }
                }
            }
            Message::Expand(section) => {
                self.expanded = if self.expanded == Some(section) { None } else { Some(section) };
            }
            Message::Font(size) => self.font = Some(size.round()),
            Message::FontDone => {
                if let Some(size) = self.font.take() {
                    if let Some(settings) = self.state.get_mut("settings").and_then(Value::as_object_mut) {
                        settings.insert("font_size".into(), json!(size as i64));
                    }
                    return self.send(json!({"action": "configure", "settings": {"font_size": size as i64}}));
                }
            }
            Message::OpenWindow => return self.open_window(),
            Message::Released => {}
            Message::Rectangle(update) => match update {
                RectangleUpdate::Rectangle(r) => self.rectangle = r.1,
                RectangleUpdate::Init(tracker) => self.tracker = Some(tracker),
            },
            Message::CloseRequested(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                    self.expanded = None;
                    return self.release();
                }
            }
            Message::TogglePopup => {
                if self.popup.is_some() {
                    return self.close_popup();
                }
                let refresh =
                    Task::perform(request(json!({"action": "state"})), |s| cosmic::Action::App(Message::State(s)));
                let open = cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                    |_| Default::default(),
                    |app: &mut Self| {
                        let id = window::Id::unique();
                        app.popup = Some(id);
                        let mut settings =
                            app.core.applet.get_popup_settings(app.core.main_window_id().unwrap(), id, None, None, None);
                        let Rectangle { x, y, width, height } = app.rectangle;
                        settings.positioner.anchor_rect = Rectangle::<i32> {
                            x: x.max(1.0) as i32,
                            y: y.max(1.0) as i32,
                            width: width.max(1.0) as i32,
                            height: height.max(1.0) as i32,
                        };
                        settings.positioner.size = None;
                        settings
                    },
                    None,
                ));
                return Task::batch([open, refresh]);
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let horizontal = matches!(self.core.applet.anchor, PanelAnchor::Top | PanelAnchor::Bottom);
        let padding = self.core.applet.suggested_padding(true).0;
        let button = button::custom(self.panel_content())
            .padding(if horizontal { [0, padding] } else { [padding, 0] })
            .on_press_down(Message::TogglePopup)
            .class(theme::Button::AppletIcon);
        // Right click starts or stops captions.
        let button = mouse_area(button).on_right_press(Message::Send(json!({"action": "toggle"})));
        let content: Element<Message> = match self.tracker.as_ref() {
            Some(tracker) => tracker.container(0, button).ignore_bounds(true).into(),
            None => button.into(),
        };
        autosize::autosize(content, AUTOSIZE_ID.clone()).into()
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Message> {
        self.core.applet.popup_container(self.menu()).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

// Keeps the caption window helper running while the applet runs; starts it again after it ends.
async fn run_captions() {
    loop {
        let child = tokio::process::Command::new(sibling("maprisma-video-translator-captions"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn();
        if let Ok(mut child) = child {
            let _ = child.wait().await;
        }
        tokio::time::sleep(RESTART_DELAY).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_and_fixes() {
        let state = json!({
            "settings": {},
            "setup": {"busy": false, "phase": "", "message": ""},
            "steps": [
                {"id": "runtime", "ok": true},
                {"id": "device", "ok": false, "problem": "Choose a device.", "fixes": [
                    {"label": "Use Speakers", "settings": {"output": "speakers"}},
                    {"label": "Capture microphone instead", "settings": {"audio_input": "Microphone", "output": ""}},
                    {"label": "Enter API key…", "panel": "provider"}
                ]}
            ]
        });
        let (problem, fixes) = next_step(&state).unwrap();
        assert_eq!(problem, "Choose a device.");
        assert_eq!(fixes.len(), 3);
        assert_eq!(fixes[0].requests, vec![json!({"action": "configure", "settings": {"output": "speakers"}})]);
        assert_eq!(fixes[1].requests.len(), 2);
        assert_eq!(fixes[1].requests[1], json!({"action": "outputs"}));
        assert!(fixes[2].panel && fixes[2].requests.is_empty());
    }

    #[test]
    fn busy_setup_and_ready() {
        let busy = json!({"setup": {"busy": true, "message": "Installing…"}, "steps": []});
        assert_eq!(next_step(&busy).unwrap().0, "Installing…");
        let ready = json!({"setup": {}, "steps": [{"id": "runtime", "ok": true}]});
        assert!(next_step(&ready).is_none());
        let failed = json!({"setup": {"phase": "error", "message": "uv failed."},
                            "steps": [{"id": "runtime", "ok": false, "problem": "Needs setup.", "fixes": [{"label": "Set up now", "action": "setup"}]}]});
        let (problem, fixes) = next_step(&failed).unwrap();
        assert_eq!(problem, "uv failed. Needs setup.");
        assert_eq!(fixes[0].requests, vec![json!({"action": "setup"})]);
    }
}
