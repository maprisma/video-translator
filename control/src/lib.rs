pub mod i18n;
pub use i18n::{tr, trf};
use serde_json::{Value, json};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
const RESTART_DELAY: Duration = Duration::from_secs(5);
pub const LANGUAGES: [&str; 18] = [
    "German",
    "English",
    "French",
    "Spanish",
    "Italian",
    "Portuguese",
    "Dutch",
    "Polish",
    "Ukrainian",
    "Russian",
    "Japanese",
    "Chinese",
    "Korean",
    "Arabic",
    "Hindi",
    "Turkish",
    "Indonesian",
    "Vietnamese",
];
pub const INPUTS: [(&str, &str); 2] = [
    (
        "Audio output",
        "Audio output · what you hear (video, calls)",
    ),
    ("Microphone", "Microphone · speech in the room"),
];

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
}

// VIDEO_TRANSLATOR_BACKEND (development) or the installed copy.
pub fn bridge_script() -> PathBuf {
    std::env::var_os("VIDEO_TRANSLATOR_BACKEND")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_home().join("maprisma-video-translator/backend"))
        .join("bridge.sh")
}

// The controller's owner-only socket (bridge.socket_path() in the backend).
pub fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    // SAFETY: getuid cannot fail.
    let uid = unsafe { libc::getuid() };
    runtime.join(format!("video-trans-{uid}.sock"))
}

pub fn sibling(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| name.into())
}

pub fn spawn(program: PathBuf) {
    let _ = std::process::Command::new(program)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

// ------------------------------------------------------------------------- controller

// One request through the bridge (one JSON line in, one out): the controller's state with the
// setup steps, or {error}. The bridge starts the controller only for actions that need it.
pub async fn request(payload: Value) -> Value {
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
            stdin
                .write_all(format!("{payload}\n").as_bytes())
                .await
                .map_err(|e| e.to_string())?;
        }
        let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        serde_json::from_str::<Value>(text.trim().lines().last().unwrap_or("{}"))
            .map_err(|e| e.to_string())
    };
    match tokio::time::timeout(Duration::from_secs(60), run).await {
        Ok(Ok(state)) => state,
        Ok(Err(error)) => json!({ "error": trf("Maprisma Video Translator: {0}", &[&error]) }),
        Err(_) => {
            json!({ "error": tr("Maprisma Video Translator: the caption controller did not answer.") })
        }
    }
}

// The light poll ({running, starting}) straight on the controller's socket, so nothing is
// started while Maprisma Video Translator is off. None when no controller runs.
pub async fn brief() -> Option<Value> {
    let path = socket_path();
    if !path.exists() {
        return None;
    }
    let ask = async {
        let mut stream = tokio::net::UnixStream::connect(path).await.ok()?;
        stream.write_all(b"{\"action\": \"brief\"}\n").await.ok()?;
        let mut line = String::new();
        BufReader::new(stream.take(65536))
            .read_line(&mut line)
            .await
            .ok()?;
        serde_json::from_str::<Value>(&line)
            .ok()
            .filter(|v| v.get("error").is_none())
    };
    tokio::time::timeout(Duration::from_secs(2), ask)
        .await
        .ok()
        .flatten()
}

pub fn flag(state: &Value, key: &str) -> bool {
    state.get(key).and_then(Value::as_bool) == Some(true)
}

pub fn string<'a>(state: &'a Value, key: &str) -> &'a str {
    state.get(key).and_then(Value::as_str).unwrap_or("")
}

pub fn setting<'a>(state: &'a Value, key: &str) -> Option<&'a Value> {
    state.get("settings").and_then(|s| s.get(key))
}

#[derive(Debug, Clone)]
pub struct Fix {
    pub label: String,
    pub requests: Vec<Value>,
    pub panel: bool,
}

// The requests a setup fix stands for ({label, action?, settings?, panel?} from readiness.steps()).
pub fn fix_requests(fix: &Value) -> Vec<Value> {
    let mut requests = Vec::new();
    if let Some(settings) = fix.get("settings").filter(|s| s.is_object()) {
        requests.push(json!({"action": "configure", "settings": settings}));
    }
    if let Some(action) = fix.get("action").and_then(Value::as_str) {
        requests.push(json!({ "action": action }));
    }
    if fix
        .get("settings")
        .and_then(|s| s.get("audio_input"))
        .is_some()
    {
        requests.push(json!({"action": "outputs"}));
    }
    requests
}

// What stands in the way of captions: the running setup, or the first failed setup step with
// the buttons that fix it. None when everything is ready.
pub fn next_step(state: &Value) -> Option<(String, Vec<Fix>)> {
    let setup = state.get("setup").unwrap_or(&Value::Null);
    if flag(setup, "busy") {
        let message = Some(string(setup, "message"))
            .filter(|m| !m.is_empty())
            .unwrap_or("Setting up…");
        return Some((tr(message), Vec::new()));
    }
    let failed = state
        .get("steps")
        .and_then(Value::as_array)?
        .iter()
        .find(|step| step.get("ok").and_then(Value::as_bool) != Some(true))?;
    let mut problem = string(failed, "problem").to_string();
    if string(setup, "phase") == "error" && string(failed, "id") == "runtime" {
        problem = format!("{} {}", tr(string(setup, "message")), tr(&problem));
    }
    let fixes = failed
        .get("fixes")
        .and_then(Value::as_array)
        .map(|fixes| {
            fixes
                .iter()
                .map(|fix| Fix {
                    label: tr(string(fix, "label")),
                    requests: fix_requests(fix),
                    // A fix with a panel ("provider") also opens the Maprisma Video Translator window.
                    panel: fix
                        .get("panel")
                        .is_some_and(|p| !p.is_null() && p != &Value::Bool(false)),
                })
                .collect()
        })
        .unwrap_or_default();
    Some((tr(&problem), fixes))
}

pub async fn run_captions() {
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

/// A check may carry a raw model identifier rather than a human-readable message.
pub fn check_detail(check: &Value, state: &Value) -> String {
    let detail = string(check, "detail");
    if ["model", "speech_model", "target"].iter().any(|key| setting(state, key).and_then(Value::as_str) == Some(detail)) {
        detail.into()
    } else {
        tr(detail)
    }
}
