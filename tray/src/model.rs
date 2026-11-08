use maprisma_video_translator_control::*;
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub requests: Vec<Value>,
    pub window: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub selected: Option<bool>,
    pub children: Vec<Item>,
    pub action: Option<Action>,
}
impl Item {
    fn new(id: i32, label: impl Into<String>, enabled: bool, requests: Vec<Value>) -> Self {
        Self {
            id,
            label: label.into(),
            enabled,
            selected: None,
            children: vec![],
            action: (!requests.is_empty()).then_some(Action {
                requests,
                window: false,
            }),
        }
    }
    pub fn find(&self, id: i32) -> Option<&Item> {
        if self.id == id {
            Some(self)
        } else {
            self.children.iter().find_map(|c| c.find(id))
        }
    }
    pub fn activate(&self, id: i32) -> Option<Action> {
        self.find(id)
            .filter(|i| i.enabled)
            .and_then(|i| i.action.clone())
    }
}
pub fn status(state: &Value) -> String {
    let setup = &state["setup"];
    let mut text = if flag(setup, "busy") || string(setup, "phase") == "error" {
        string(setup, "message")
    } else {
        ""
    }
    .to_owned();
    if text.is_empty() {
        text = string(state, "status").to_owned();
    }
    if text.is_empty() {
        text = "Maprisma Video Translator".into();
    }
    if let Some(checks) = state["checks"].as_array() {
        for c in checks.iter().filter(|c| string(c, "status") == "error") {
            text.push_str(&format!(
                "\n✕ {}: {}",
                string(c, "label"),
                string(c, "detail")
            ));
        }
    }
    text
}
pub fn menu(state: &Value, busy: bool) -> Item {
    let loaded = state.get("settings").is_some();
    let steps = state["steps"].as_array().cloned().unwrap_or_default();
    let runtime = loaded
        && !steps
            .iter()
            .any(|s| string(s, "id") == "runtime" && !flag(s, "ok"));
    let ready = loaded && !steps.is_empty() && steps.iter().all(|s| flag(s, "ok"));
    let running = flag(state, "running");
    let control = |id, label, action, enabled| {
        Item::new(id, label, enabled && !busy, vec![json!({"action":action})])
    };
    let mut items = vec![
        Item::new(1, status(state), false, vec![]),
        control(
            2,
            "Start",
            "start",
            ready
                && !running
                && !flag(state, "stopping")
                && !flag(state, "downloading")
                && !flag(&state["setup"], "busy"),
        ),
        control(
            3,
            "Stop",
            "stop",
            (running || flag(state, "overlay")) && !flag(state, "stopping"),
        ),
        control(4, "Captions", "show", runtime),
    ];
    if loaded {
        if let Some((problem, fixes)) = next_step(state) {
            items.push(Item::new(5, problem, false, vec![]));
            for (i, fix) in fixes.into_iter().enumerate() {
                let mut item = Item::new(1000 + i as i32, fix.label, !busy, vec![]);
                item.action = Some(Action {
                    requests: fix.requests,
                    window: fix.panel,
                });
                items.push(item);
            }
        }
    }
    if flag(state, "downloading") {
        items.push(control(6, "Cancel download", "cancel-download", true));
    }
    // Language, input, following and font size live in the settings window.
    if loaded {
        items.push(control(14, "Clear captions", "clear", runtime));
    }
    let mut open = Item::new(15, "Settings…", true, vec![]);
    open.action = Some(Action {
        requests: vec![],
        window: true,
    });
    items.push(open);
    Item {
        id: 0,
        label: String::new(),
        enabled: true,
        selected: None,
        children: items,
        action: None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn ready() -> Value {
        json!({"settings":{},"steps":[{"id":"runtime","ok":true}]})
    }
    #[test]
    fn readiness_and_running() {
        assert!(menu(&Value::Null, false).activate(2).is_none());
        let mut state = ready();
        assert_eq!(
            menu(&state, false).activate(2).unwrap().requests,
            vec![json!({"action":"start"})]
        );
        state["running"] = json!(true);
        let m = menu(&state, false);
        assert!(m.activate(2).is_none());
        assert!(m.activate(3).is_some());
        assert!(menu(&ready(), true).activate(2).is_none());
    }
    #[test]
    fn settings_and_fixes() {
        let mut state = ready();
        let m = menu(&state, false);
        // Only the controls and Settings…: the options are in the window.
        let labels: Vec<_> = m.children.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels[1..],
            ["Start", "Stop", "Captions", "Clear captions", "Settings…"]
        );
        for id in [10, 11, 12, 13, 101, 201, 332] {
            assert!(m.find(id).is_none());
        }
        assert!(m.activate(15).unwrap().window);
        assert!(m.activate(-1).is_none());
        state["steps"] = json!([{"id":"provider","ok":false,"problem":"Key needed","fixes":[{"label":"Enter key","panel":"provider","settings":{"provider":"Online"}}]}]);
        let a = menu(&state, false).activate(1000).unwrap();
        assert!(a.window);
        assert_eq!(a.requests[0]["settings"]["provider"], "Online");
    }
}
