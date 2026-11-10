//! Shared, bundled interface translations. The Python backend remains language-neutral.
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf, sync::LazyLock};
const CODES: [&str; 8] = ["en", "de", "fr", "es", "it", "pt", "nl", "pl"];
static CATALOGS: LazyLock<HashMap<String, HashMap<String, String>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("translations.json")).expect("generated catalogs")
});
pub fn system_language(env: &HashMap<String, String>) -> String {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|key| env.get(*key))
        .find(|s| !s.is_empty())
        .map(String::as_str)
        .unwrap_or("");
    let code = locale
        .split(['_', '.', '-', '@'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    if CODES.contains(&code.as_str()) {
        code
    } else {
        "en".into()
    }
}
fn settings_path() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".config"));
    config.join("maprisma-video-translator/interface.json")
}
/// Settings › F12 saves a screenshot (on unless `"screenshot_key": false`) and › Corners
/// (`"corners"`: "desktop", "slight", "rounded", "square"), set in the window, kept in the same
/// file as the language.
pub fn look_settings() -> (bool, String) {
    let saved = std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(Value::Null);
    (
        saved.get("screenshot_key").and_then(Value::as_bool) != Some(false),
        saved.get("corners").and_then(Value::as_str).unwrap_or("").to_string(),
    )
}
pub fn language() -> String {
    let selected = std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v["language"].as_str().map(str::to_owned))
        .unwrap_or_default();
    if CODES.contains(&selected.as_str()) {
        selected
    } else {
        system_language(&std::env::vars().collect())
    }
}
pub fn tr(english: &str) -> String {
    let catalog = &CATALOGS[&language()];
    if let Some(text) = catalog.get(english) {
        return text.clone();
    }
    // Match backend templates without interpreting or translating captured data.
    let mut templates: Vec<_> = catalog.iter().collect();
    templates.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
    for (key, translation) in templates {
        if !key.contains("{0}") {
            continue;
        }
        if let Some(mut values) = capture(key, english) {
            for (index, value) in values.iter_mut().enumerate() {
                if nested(key, index) {
                    *value = tr(value);
                }
            }
            return substitute(
                translation,
                &values.iter().map(String::as_str).collect::<Vec<_>>(),
            );
        }
    }
    english.into()
}
fn capture(template: &str, text: &str) -> Option<Vec<String>> {
    let mut remaining = text;
    let mut tail = template;
    let mut values = Vec::new();
    for index in 0.. {
        let marker = format!("{{{index}}}");
        let Some((prefix, after)) = tail.split_once(&marker) else {
            return (remaining == tail).then_some(values);
        };
        remaining = remaining.strip_prefix(prefix)?;
        let next = format!("{{{}}}", index + 1);
        let suffix = after.split_once(&next).map(|p| p.0).unwrap_or(after);
        let length = if after.contains(&next) {
            remaining.find(suffix)?
        } else {
            remaining.strip_suffix(suffix)?.len()
        };
        values.push(remaining[..length].into());
        remaining = &remaining[length..];
        tail = after;
    }
    unreachable!()
}
fn substitute(template: &str, values: &[&str]) -> String {
    let mut output = String::new();
    let mut tail = template;
    while let Some(start) = tail.find('{') {
        output.push_str(&tail[..start]);
        let Some(end) = tail[start..].find('}').map(|end| start + end) else {
            output.push_str(&tail[start..]);
            return output;
        };
        let slot = &tail[start..=end];
        let value = tail[start + 1..end]
            .parse::<usize>()
            .ok()
            .and_then(|index| values.get(index));
        output.push_str(value.copied().unwrap_or(slot));
        tail = &tail[end + 1..];
    }
    output.push_str(tail);
    output
}

pub fn trf(english: &str, values: &[&str]) -> String {
    substitute(&tr(english), values)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_precedence_and_fallback() {
        for (all, messages, lang, expected) in [
            ("", "", "de_DE.UTF-8", "de"),
            ("xx_YY", "fr_FR", "de", "en"),
            ("", "pl_PL", "de", "pl"),
            ("C", "fr", "de", "en"),
            ("", "", "", "en"),
        ] {
            let env = [("LC_ALL", all), ("LC_MESSAGES", messages), ("LANG", lang)]
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect();
            assert_eq!(system_language(&env), expected);
        }
    }
    #[test]
    fn template_values_are_preserved() {
        assert_eq!(
            substitute("{0} {1}", &["device {1}", "value"]),
            "device {1} value"
        );
        assert_eq!(
            capture(
                "Enter the API key or bearer token for {0}.",
                "Enter the API key or bearer token for https://example.com/v1."
            ),
            Some(vec!["https://example.com/v1".into()])
        );
        assert_eq!(
            capture(
                "Download Whisper {0} ({1})",
                "Download Whisper small (480 MB)"
            ),
            Some(vec!["small".into(), "480 MB".into()])
        );
    }
}

fn nested(key: &str, index: usize) -> bool {
    matches!(
        key,
        "Maprisma Video Translator needs a one-time setup: {0}."
            | "{0} and {1}"
            | "{0} · worker stopped"
            | "Runtime setup failed: {0}"
    ) || (key == "{0} is not installed or has no user service. {1}" && index == 1)
        || key == "Enter the API key or bearer token for {0}."
}
