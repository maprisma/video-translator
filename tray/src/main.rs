mod model;
use maprisma_video_translator_control::*;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{Mutex, mpsc};
use zbus::{
    Connection,
    object_server::SignalEmitter,
    zvariant::{OwnedObjectPath, OwnedValue, Value as Variant},
};

type Props = HashMap<String, OwnedValue>;
type Layout = (i32, Props, Vec<OwnedValue>);
type Pixmaps = Vec<(i32, i32, Vec<u8>)>;
const ICON: &str = "io.github.ubruckhaus.CosmicMaprismaVideoTranslator-symbolic";
/// The filled subtitles in the theme accent (ColorScheme-Highlight) while captions run.
const ICON_ACTIVE: &str = "io.github.ubruckhaus.CosmicMaprismaVideoTranslator-active-symbolic";
/// The outline subtitles while no captions run (data/icons/…-off-symbolic.svg).
const ICON_OFF: &str = "io.github.ubruckhaus.CosmicMaprismaVideoTranslator-off-symbolic";
/// The bar icon, as the applet draws it: green while captions run, else the outline.
fn icon_for(value: &Value) -> &'static str {
    if flag(value, "running") { ICON_ACTIVE } else { ICON_OFF }
}
struct State {
    value: Value,
    busy: bool,
    revision: u32,
    language: String,
}
type Shared = Arc<Mutex<State>>;
#[derive(Clone)]
struct Menu {
    state: Shared,
    tx: mpsc::UnboundedSender<model::Action>,
}
fn owned<T: Into<Variant<'static>>>(value: T) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}
fn properties(item: &model::Item, names: &[String]) -> Props {
    let mut props = Props::new();
    props.insert("label".into(), owned(item.label.clone()));
    props.insert("enabled".into(), owned(item.enabled));
    props.insert("visible".into(), owned(true));
    if !item.children.is_empty() {
        props.insert("children-display".into(), owned("submenu"));
    }
    if let Some(selected) = item.selected {
        props.insert(
            "toggle-type".into(),
            owned(if item.id == 12 { "checkmark" } else { "radio" }),
        );
        props.insert("toggle-state".into(), owned(i32::from(selected)));
    }
    if !names.is_empty() {
        props.retain(|key, _| names.contains(key));
    }
    props
}
fn layout(item: &model::Item, depth: i32, names: &[String]) -> Layout {
    (
        item.id,
        properties(item, names),
        if depth == 0 {
            vec![]
        } else {
            item.children
                .iter()
                .map(|c| {
                    OwnedValue::try_from(Variant::new(layout(
                        c,
                        if depth < 0 { -1 } else { depth - 1 },
                        names,
                    )))
                    .unwrap()
                })
                .collect()
        },
    )
}
impl Menu {
    async fn dispatch(&self, id: i32, event: &str) -> bool {
        let s = self.state.lock().await;
        let menu = model::menu(&s.value, s.busy);
        let Some(item) = menu.find(id) else {
            return false;
        };
        if event == "closed" && id == 0 {
            let _ = self.tx.send(model::Action {
                requests: vec![json!({"action":"release"})],
                window: false,
                toggle: false,
            });
        }
        if event == "clicked" {
            if let Some(action) = menu.activate(item.id) {
                let _ = self.tx.send(action);
            }
        }
        true
    }
}
#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }
    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }
    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }
    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        vec![]
    }
    async fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> zbus::fdo::Result<(u32, Layout)> {
        let s = self.state.lock().await;
        let menu = model::menu(&s.value, s.busy);
        let item = menu
            .find(parent_id)
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("Unknown menu ID".into()))?;
        Ok((s.revision, layout(item, recursion_depth, &property_names)))
    }
    async fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> Vec<(i32, Props)> {
        let s = self.state.lock().await;
        let menu = model::menu(&s.value, s.busy);
        ids.iter()
            .filter_map(|id| {
                menu.find(*id)
                    .map(|i| (*id, properties(i, &property_names)))
            })
            .collect()
    }
    async fn get_property(&self, id: i32, name: String) -> zbus::fdo::Result<OwnedValue> {
        let s = self.state.lock().await;
        let menu = model::menu(&s.value, s.busy);
        menu.find(id)
            .and_then(|i| properties(i, &[]).remove(&name))
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("Unknown menu property".into()))
    }
    async fn event(&self, id: i32, event_id: String, _data: OwnedValue, _timestamp: u32) {
        self.dispatch(id, &event_id).await;
    }
    async fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        let mut errors = vec![];
        for (id, event, _, _) in events {
            if !self.dispatch(id, &event).await {
                errors.push(id);
            }
        }
        errors
    }
    async fn about_to_show(&self, _id: i32) -> bool {
        false
    }
    async fn about_to_show_group(&self, ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        let s = self.state.lock().await;
        let menu = model::menu(&s.value, s.busy);
        (
            vec![],
            ids.into_iter()
                .filter(|id| menu.find(*id).is_none())
                .collect(),
        )
    }
    #[zbus(signal)]
    async fn layout_updated(
        emitter: &SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
}
struct Tray {
    state: Shared,
    tx: mpsc::UnboundedSender<model::Action>,
}
#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Tray {
    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }
    #[zbus(property)]
    fn id(&self) -> &str {
        "maprisma-video-translator"
    }
    #[zbus(property)]
    fn title(&self) -> String {
        tr("Maprisma Video Translator")
    }
    #[zbus(property)]
    async fn status(&self) -> &str {
        if flag(&self.state.lock().await.value, "running") {
            "NeedsAttention"
        } else {
            "Active"
        }
    }
    #[zbus(property)]
    fn window_id(&self) -> u32 {
        0
    }
    #[zbus(property)]
    async fn icon_name(&self) -> &str {
        icon_for(&self.state.lock().await.value)
    }
    #[zbus(property)]
    fn icon_pixmap(&self) -> Pixmaps {
        vec![]
    }
    #[zbus(property)]
    fn overlay_icon_name(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn overlay_icon_pixmap(&self) -> Pixmaps {
        vec![]
    }
    #[zbus(property)]
    fn attention_icon_name(&self) -> &str {
        ICON_ACTIVE
    }
    #[zbus(property)]
    fn attention_icon_pixmap(&self) -> Pixmaps {
        vec![]
    }
    #[zbus(property)]
    fn attention_movie_name(&self) -> &str {
        ""
    }
    #[zbus(property)]
    async fn tool_tip(&self) -> (String, Pixmaps, String, String) {
        (
            ICON.into(),
            vec![],
            tr("Maprisma Video Translator"),
            model::status(&self.state.lock().await.value),
        )
    }
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from("/MenuBar").unwrap()
    }
    fn activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.send(model::Action {
            requests: vec![],
            window: true,
            toggle: true,
        });
    }
    fn secondary_activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.send(model::Action {
            requests: vec![json!({"action":"toggle"})],
            window: false,
            toggle: false,
        });
    }
    fn context_menu(&self, _x: i32, _y: i32) {} // The host displays the exported Menu.
    fn scroll(&self, _delta: i32, _orientation: String) {}
    #[zbus(signal)]
    async fn new_status(emitter: &SignalEmitter<'_>, status: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn new_tool_tip(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn new_icon(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
async fn publish(
    connection: &Connection,
    state: &Shared,
    value: Value,
    busy: bool,
) -> zbus::Result<()> {
    let mut s = state.lock().await;
    let before = model::menu(&s.value, s.busy);
    let old_status = flag(&s.value, "running");
    s.value = value;
    s.busy = busy;
    let language = i18n::language();
    let changed = s.language != language || before != model::menu(&s.value, s.busy);
    s.language = language;
    if changed {
        s.revision = s.revision.wrapping_add(1);
    }
    let revision = s.revision;
    let running = flag(&s.value, "running");
    drop(s);
    if changed {
        Menu::layout_updated(&SignalEmitter::new(connection, "/MenuBar")?, revision, 0).await?;
        Tray::new_tool_tip(&SignalEmitter::new(connection, "/StatusNotifierItem")?).await?;
    }
    if running != old_status {
        Tray::new_icon(&SignalEmitter::new(connection, "/StatusNotifierItem")?).await?;
        Tray::new_status(
            &SignalEmitter::new(connection, "/StatusNotifierItem")?,
            if running { "NeedsAttention" } else { "Active" },
        )
        .await?;
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(Mutex::new(State {
        value: Value::Null,
        busy: false,
        revision: 1,
            language: i18n::language(),
    }));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let connection = zbus::connection::Builder::session()?
        .serve_at(
            "/StatusNotifierItem",
            Tray {
                state: state.clone(),
                tx: tx.clone(),
            },
        )?
        .serve_at(
            "/MenuBar",
            Menu {
                state: state.clone(),
                tx,
            },
        )?
        .build()
        .await?;
    // One tray per session: a second one ends here (zbus would otherwise
    // queue the name request and keep running, with a second caption helper).
    if connection.request_name_with_flags("com.maprisma.VideoTranslator.Tray", zbus::fdo::RequestNameFlags::DoNotQueue.into()).await.is_err() {
        return Ok(());
    }
    let _captions = tokio::spawn(run_captions());
    // Checking the unique owner also handles Plasma starting after us and watcher restarts.
    let watcher_connection = connection.clone();
    tokio::spawn(async move {
        let mut registered = String::new();
        loop {
            let dbus = zbus::fdo::DBusProxy::new(&watcher_connection).await;
            let owner = match dbus {
                Ok(p) => p
                    .get_name_owner("org.kde.StatusNotifierWatcher".try_into().unwrap())
                    .await
                    .ok()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                Err(_) => String::new(),
            };
            if owner.is_empty() {
                registered.clear();
            } else if owner != registered {
                if let Ok(proxy) = zbus::Proxy::new(
                    &watcher_connection,
                    owner.as_str(),
                    "/StatusNotifierWatcher",
                    "org.kde.StatusNotifierWatcher",
                )
                .await
                {
                    if proxy
                        .call::<_, _, ()>(
                            "RegisterStatusNotifierItem",
                            &(watcher_connection.unique_name().unwrap().as_str(),),
                        )
                        .await
                        .is_ok()
                    {
                        registered = owner;
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
    let mut poll = tokio::time::interval(Duration::from_millis(1500));
    loop {
        tokio::select! {
            _=poll.tick()=> {
                let value=request(json!({"action":"state"})).await;
                if value.get("settings").is_some() {publish(&connection,&state,value,false).await?;}
            }
            Some(action)=rx.recv()=> {
                let value=state.lock().await.value.clone(); publish(&connection,&state,value,true).await?;
                for payload in action.requests {
                    let value=request(payload).await;
                    if let Some(error)=value["error"].as_str() {eprintln!("{error}");}
                    else if value.get("settings").is_some() {publish(&connection,&state,value,true).await?;}
                }
                if action.window {
                    let app = sibling("maprisma-video-translator");
                    if action.toggle {spawn_args(app, &["--toggle"]);} else {spawn(app);}
                }
                let value=state.lock().await.value.clone(); publish(&connection,&state,value,false).await?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbus_layout_signature_and_depth() {
        let menu = model::menu(&json!({"settings":{},"steps":[]}), false);
        let tree = layout(&menu, -1, &[]);
        assert_eq!(Variant::new(tree).value_signature().to_string(), "(ia{sv}av)");
        assert!(layout(&menu, 0, &[]).2.is_empty());
        assert!(!layout(&menu, 1, &[]).2.is_empty());
        // The menu has no check items now; the property code still supports them.
        let mut check = menu.find(2).unwrap().clone();
        check.selected = Some(true);
        let filtered = properties(&check, &["toggle-state".into()]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(i32::try_from(filtered["toggle-state"].try_clone().unwrap()).unwrap(), 1);
    }

    #[test]
    fn bar_icon_follows_the_captions() {
        assert_eq!(icon_for(&json!({"running":true})), ICON_ACTIVE);
        assert!(ICON_ACTIVE.ends_with("-active-symbolic"), "Maprisma Shell tints -active-symbolic icons");
        let svg = include_str!("../../data/icons/io.github.ubruckhaus.CosmicMaprismaVideoTranslator-active-symbolic.svg");
        assert!(svg.contains("ColorScheme-Highlight") && !svg.contains("ColorScheme-Text {"));
        assert_eq!(icon_for(&json!({"running":false,"overlay":true})), ICON_OFF);
        assert_eq!(icon_for(&Value::Null), ICON_OFF);
    }

    #[tokio::test]
    async fn left_click_toggles_the_window_middle_click_captions() {
        let state = Arc::new(Mutex::new(State { value: Value::Null, busy: false, revision: 1, language: i18n::language() }));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tray = Tray { state, tx };
        tray.activate(0, 0);
        assert_eq!(rx.try_recv().unwrap(), model::Action { requests: vec![], window: true, toggle: true });
        tray.secondary_activate(0, 0);
        let middle = rx.try_recv().unwrap();
        assert_eq!(middle.requests, vec![json!({"action":"toggle"})]);
        assert!(!middle.window);
    }

    #[tokio::test]
    async fn events_and_event_groups_without_bus() {
        let state = Arc::new(Mutex::new(State {
            value: json!({"settings":{},"steps":[{"id":"runtime","ok":true}]}),
            busy: false, revision: 1,
            language: i18n::language(),
        }));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let menu = Menu { state, tx };
        menu.event(2, "clicked".into(), owned(0i32), 0).await;
        assert_eq!(rx.try_recv().unwrap().requests, vec![json!({"action":"start"})]);
        let errors = menu.event_group(vec![
            (15,"clicked".into(),owned(0i32),0),
            (-1,"clicked".into(),owned(0i32),0),
            (3,"clicked".into(),owned(0i32),0), // disabled
        ]).await;
        assert_eq!(errors, vec![-1]);
        assert!(rx.try_recv().unwrap().window);
        assert!(rx.try_recv().is_err());
        menu.event(0,"closed".into(),owned(0i32),0).await;
        assert_eq!(rx.try_recv().unwrap().requests,vec![json!({"action":"release"})]);
    }
}
