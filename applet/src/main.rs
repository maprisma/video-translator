// Maprisma Video Translator: the subtitles icon in the panel. Left click opens the menu (again:
// closes it; Esc closes an open drop-down, then the menu)
// (Start / Stop / Captions, what needs setting up, target language, input, caption size and the
// Maprisma Video Translator window); right click starts or stops captions. A green line under the icon
// while captions run.
//
// The caption controller (backend/gui.py, hidden) is reached through backend/bridge.sh like the
// GNOME extension did; the applet also keeps the caption window helper
// (maprisma-video-translator-captions) running, which draws the captions while the controller shows them.

use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::applet::cosmic_panel_config::PanelAnchor;
use cosmic::applet::{menu_button, padded_control};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::destroy_popup;
use cosmic::iced::widget::{container as iced_container, svg as iced_svg, text as iced_text};
use cosmic::iced::{Alignment, Background, Border, Color, Length, Rectangle, Subscription, stream, window};
use cosmic::widget::rectangle_tracker::{RectangleTracker, RectangleUpdate, rectangle_tracker_subscription};
use cosmic::widget::{
    Id, autosize, button, column, container, divider, icon, mouse_area, row, scrollable, slider, space, text, toggler,
};
use cosmic::{Element, theme};
use serde_json::{Value, json};
use maprisma_video_translator_control::*;
#[allow(dead_code)] // the shared copy of every Maprisma COSMIC app; not every part is used here
mod corners;
mod look;
mod shot;
mod shot_files;
use shot::{Shot, ShotMsg};

const APP_ID: &str = "io.github.ubruckhaus.CosmicMaprismaVideoTranslator";
const ICON: &str = "io.github.ubruckhaus.CosmicMaprismaVideoTranslator-symbolic";
/// The bar's outline subtitles while no captions run (as the tray shows it).
const ICON_OFF: &str = "io.github.ubruckhaus.CosmicMaprismaVideoTranslator-off-symbolic";
const POPUP_WIDTH: f32 = 360.0;
/// F12's folder in ~/Pictures and (as a slug) its file names (the window's too).
const SHOT_FOLDER: &str = "Maprisma Video Translator";
const FONT: (f32, f32) = (12.0, 64.0);
static AUTOSIZE_ID: LazyLock<Id> = LazyLock::new(|| Id::new("maprisma-video-translator-autosize"));


fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<Translator>(())
}

// ------------------------------------------------------------------------- paths

fn notify(text: &str) {
    let _ = std::process::Command::new("notify-send")
        .args(["--app-name=Maprisma Video Translator", "-i", ICON, &tr("Maprisma Video Translator"), &tr(text)])
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

/// The line under the panel icon while translating: the theme's success colour (the desktop's
/// green).
// The running subtitles: the desktop's accent, as the tray's -active-symbolic icon.
fn accent_svg() -> theme::Svg {
    theme::Svg::custom(|theme: &cosmic::Theme| iced_svg::Style { color: Some(theme.cosmic().accent_color().into()) })
}

fn running_line(radius: f32) -> theme::Container<'static> {
    theme::Container::custom(move |theme| iced_container::Style {
        background: Some(Background::Color(theme.cosmic().success_color().into())),
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
    shot: Shot,
    corners: String, // Settings › Corners (corners.rs)
    screenshot_key: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Shot(ShotMsg),
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

/// What Esc in a surface of the applet does, innermost first: the screenshot note (shot.rs skips
/// it), an open drop-down (target language, input), then the popup; elsewhere nothing.
#[derive(Debug, PartialEq)]
enum Escape {
    Note,
    Collapse,
    ClosePopup,
    Nothing,
}

fn escape(id: window::Id, popup: Option<window::Id>, note: bool, expanded: bool) -> Escape {
    if note {
        Escape::Note
    } else if popup != Some(id) {
        Escape::Nothing
    } else if expanded {
        Escape::Collapse
    } else {
        Escape::ClosePopup
    }
}

impl Translator {
    fn settings_font(&self) -> f32 {
        setting(&self.state, "font_size").and_then(Value::as_f64).unwrap_or(24.0).clamp(12.0, 64.0) as f32
    }

    fn panel_content(&self) -> Element<'_, Message> {
        let size = self.core.applet.suggested_size(true).0;
        let subtitles = icon::from_name(if self.running { ICON } else { ICON_OFF }).size(size).symbolic(true).icon();
        let subtitles = if self.running { subtitles.class(accent_svg()) } else { subtitles };
        let line: Element<Message> = if self.running {
            container(space::horizontal())
                .width(Length::Fixed(size as f32))
                .height(Length::Fixed(2.0))
                .class(running_line(corners::now().small.min(1.0)))
                .into()
        } else {
            space::vertical().height(Length::Fixed(2.0)).into()
        };
        // 2 px: the panel's icon slot (not a layout gap), as the Omarchy original.
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
                status.push(format!("✕ {}: {}", tr(string(check, "label")), check_detail(check, state)));
            }
        }
        let header = row![icon::from_name(ICON).size(24).symbolic(true).icon(), text::body(status.iter().map(|s| tr(s)).collect::<Vec<_>>().join("\n")).width(Length::Fill)]
            .spacing(12)
            .align_y(Alignment::Center);

        let control = |icon_name: &'static str, label: String, action: &'static str, enabled: bool| {
            button::standard(label)
                .leading_icon(icon::from_name(icon_name).size(16).symbolic(true))
                .width(Length::Fill)
                .on_press_maybe((enabled && idle).then(|| Message::Send(json!({ "action": action }))))
        };
        let controls = row![
            control(
                "media-playback-start-symbolic",
                tr("Start"),
                "start",
                ready && !self.running && !stopping && !downloading && !flag(setup, "busy")
            ),
            control("media-playback-stop-symbolic", tr("Stop"), "stop", (self.running || flag(state, "overlay")) && !stopping),
            control(ICON, tr("Captions"), "show", runtime_ready),
        ]
        .spacing(8);

        let mut top = column![header, controls].spacing(12);
        if let Some((problem, fixes)) = loaded.then(|| next_step(state)).flatten() {
            let mut block = column![text::caption(problem).class(theme::Text::Custom(dim_text))].spacing(8);
            for (index, fix) in fixes.iter().enumerate() {
                let face = if index == 0 { button::suggested(fix.label.clone()) } else { button::standard(fix.label.clone()) };
                block = block.push(face.width(Length::Fill).on_press_maybe(idle.then_some(Message::Fix(index))));
            }
            top = top.push(block);
        }
        if downloading {
            top = top.push(
                button::standard(tr("Cancel download"))
                    .width(Length::Fill)
                    .on_press(Message::Send(json!({"action": "cancel-download"}))),
            );
        }

        let mut actions = column![divider::horizontal::default()];
        if loaded {
            let target = setting(state, "target").and_then(Value::as_str).unwrap_or("German");
            let input = setting(state, "audio_input").and_then(Value::as_str).unwrap_or("Audio output");
            actions = actions.push(self.expander(Section::Target, trf("Translate into · {0}", &[&tr(target)]), !locked));
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
            actions = actions.push(self.expander(Section::Input, trf("Listen to · {0}", &[&tr(input)]), runtime_ready && !locked));
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
                    text::body(tr("Follow new captions")).width(Length::Fill),
                    toggler(follow).on_toggle(|on| Message::Send(json!({"action": "configure", "settings": {"auto_scroll": on}}))),
                ]
                .align_y(Alignment::Center),
            ));
            let font = self.font.unwrap_or_else(|| self.settings_font());
            actions = actions.push(padded_control(
                row![
                    icon::from_name("preferences-desktop-font-symbolic").size(16).symbolic(true),
                    slider(FONT.0..=FONT.1, font, Message::Font).step(1.0_f32).on_release(Message::FontDone),
                    text::body(trf("{0} pt", &[&format!("{font:.0}")])).width(Length::Fixed(48.0)),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            ));
            actions = actions.push(
                menu_button(text::body(tr("Clear captions")))
                    .on_press_maybe(runtime_ready.then(|| Message::Send(json!({"action": "clear"})))),
            );
        }
        actions = actions
            .push(divider::horizontal::default())
            .push(menu_button(text::body(tr("Open Maprisma Video Translator…"))).on_press(Message::OpenWindow));

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
        menu_button(row![mark, text::body(tr(label))].spacing(8).padding([0, 0, 0, 12]).align_y(Alignment::Center))
            .on_press_maybe((!selected).then_some(Message::Send(payload)))
            .into()
    }

    fn read_look(&mut self) -> Task<Message> {
        let (screenshot_key, corners) = i18n::look_settings();
        self.screenshot_key = screenshot_key;
        self.corners = corners::normalize(&corners).to_string();
        corners::set_current(&self.corners);
        // The desktop's colours and corners (Maprisma Theme can change them any time); the
        // popup's view reads them (corners::apply), this only re-reads them (at most every 5 s).
        look::refresh();
        Task::none()
    }

    fn close_popup(&mut self) -> Task<Message> {
        self.expanded = None;
        match self.popup.take() {
            Some(popup) => {
                self.shot.closed(popup);
                Task::batch([destroy_popup(popup), self.release()])
            }
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
        let mut app = Self { core, shot: Shot::new(SHOT_FOLDER), ..Default::default() };
        let look = app.read_look();
        (app, Task::batch([look, Task::done(cosmic::Action::App(Message::Poll))]))
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::CloseRequested(id))
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            rectangle_tracker_subscription(0).map(|update| Message::Rectangle(update.1)),
            cosmic::iced::time::every(Duration::from_millis(1500)).map(|_| Message::Poll),
            Shot::subscription().map(Message::Shot),
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
                let look = self.read_look();
                if self.popup.is_some() {
                    if self.busy == 0 {
                        let state = Task::perform(request(json!({"action": "state"})), |s| {
                            cosmic::Action::App(Message::State(s))
                        });
                        return Task::batch([look, state]);
                    }
                } else {
                    let brief = Task::perform(brief(), |s| cosmic::Action::App(Message::Brief(s)));
                    return Task::batch([look, brief]);
                }
                return look;
            }
            Message::Shot(ShotMsg::Escape(id)) => {
                return match escape(id, self.popup, self.shot.showing(id), self.expanded.is_some()) {
                    Escape::Note => self.shot.update(ShotMsg::Escape(id), self.screenshot_key, true, Message::Shot),
                    Escape::Collapse => {
                        self.expanded = None;
                        Task::none()
                    }
                    Escape::ClosePopup => self.close_popup(),
                    Escape::Nothing => Task::none(),
                };
            }
            Message::Shot(message) => {
                // F12 takes the open popup, not the panel button.
                let ours = match &message {
                    ShotMsg::Key(id) => self.popup == Some(*id),
                    _ => true,
                };
                return self.shot.update(message, self.screenshot_key, ours, Message::Shot);
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
                self.shot.closed(id);
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

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        let content = match self.shot.card(id, Message::Shot) {
            Some(card) => column![card, self.menu()].width(Length::Fixed(POPUP_WIDTH)).into(),
            None => self.menu(),
        };
        corners::apply(&self.corners, self.core.applet.popup_container(content))
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
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

    #[test]
    fn escape_goes_from_the_innermost_outwards() {
        let popup = window::Id::unique();
        let other = window::Id::unique();
        assert_eq!(escape(popup, Some(popup), true, true), Escape::Note);
        assert_eq!(escape(popup, Some(popup), false, true), Escape::Collapse);
        assert_eq!(escape(popup, Some(popup), false, false), Escape::ClosePopup);
        assert_eq!(escape(other, Some(popup), false, true), Escape::Nothing);
        assert_eq!(escape(popup, None, false, false), Escape::Nothing);
    }
}
