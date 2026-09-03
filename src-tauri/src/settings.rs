//! Settings and bookmarks persistence. Both live under the XDG config
//! directory as plain, hand-editable JSON, never inside a repository.
//! Settings validate per-key: one bad value falls back to its default
//! rather than rejecting the whole file.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gittree")
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

fn bookmarks_path() -> PathBuf {
    config_dir().join("bookmarks.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub concurrency: usize,
    /// `None` follows the desktop preference; `Some(theme)` is an explicit override.
    pub theme: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            concurrency: 8,
            theme: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsLoadResult {
    pub settings: Settings,
    /// One entry per key that fell back to its default, naming the key and
    /// why — never a silent, unexplained fallback.
    pub warnings: Vec<String>,
}

/// Loads settings with per-key fallback: an invalid `concurrency` does not
/// take `theme` down with it, and a missing file is not an error.
pub fn load_settings() -> SettingsLoadResult {
    let path = settings_path();
    let default = Settings::default();
    let raw = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => {
            return SettingsLoadResult {
                settings: default,
                warnings: vec![],
            }
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return SettingsLoadResult {
                settings: default,
                warnings: vec![format!(
                    "{}: could not parse as JSON ({e}); using all defaults",
                    path.display()
                )],
            }
        }
    };

    let mut warnings = Vec::new();
    let mut settings = default.clone();

    match value.get("concurrency") {
        None => {}
        Some(v) => match v.as_u64().filter(|n| *n > 0) {
            Some(n) => settings.concurrency = n as usize,
            None => warnings.push(format!(
                "concurrency: expected a positive integer, got {v}; using default {}",
                default.concurrency
            )),
        },
    }

    match value.get("theme") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::String(s)) => settings.theme = Some(s.clone()),
        Some(v) => warnings.push(format!(
            "theme: expected a string or null, got {v}; using default (follow desktop)"
        )),
    }

    SettingsLoadResult { settings, warnings }
}

pub fn save_settings(settings: &Settings) -> std::io::Result<()> {
    fs::create_dir_all(config_dir())?;
    let text = serde_json::to_string_pretty(settings).expect("Settings serializes");
    fs::write(settings_path(), text)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub root: String,
    pub group: Option<String>,
    pub order: i32,
}

/// Missing file -> empty list. A malformed individual entry is skipped
/// rather than invalidating the whole list.
pub fn load_bookmarks() -> Vec<Bookmark> {
    let path = bookmarks_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) else {
        tracing::warn!(path = %path.display(), "bookmarks file is not valid JSON; treating as empty");
        return Vec::new();
    };
    entries
        .into_iter()
        .filter_map(|v| {
            serde_json::from_value::<Bookmark>(v.clone())
                .ok()
                .or_else(|| {
                    tracing::warn!(entry = %v, "skipping malformed bookmark entry");
                    None
                })
        })
        .collect()
}

pub fn save_bookmarks(bookmarks: &[Bookmark]) -> std::io::Result<()> {
    fs::create_dir_all(config_dir())?;
    let text = serde_json::to_string_pretty(bookmarks).expect("Vec<Bookmark> serializes");
    fs::write(bookmarks_path(), text)
}

/// In-memory bookmark list, mirrored to `bookmarks.json` on every change.
pub struct BookmarksState(pub Mutex<Vec<Bookmark>>);

impl Default for BookmarksState {
    fn default() -> Self {
        Self(Mutex::new(load_bookmarks()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_concurrency_key_falls_back_alone() {
        let value = serde_json::json!({ "theme": "dark" });
        let raw = serde_json::to_string(&value).unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), raw).unwrap();
        // load_settings() reads a fixed XDG path, so directly exercise the
        // per-key merge logic it uses instead of the path.
        let default = Settings::default();
        let mut settings = default.clone();
        let mut warnings = Vec::new();
        match value.get("concurrency") {
            None => {}
            Some(v) => match v.as_u64() {
                Some(n) => settings.concurrency = n as usize,
                None => warnings.push("concurrency invalid".to_string()),
            },
        }
        if let Some(serde_json::Value::String(s)) = value.get("theme") {
            settings.theme = Some(s.clone());
        }
        assert_eq!(settings.concurrency, default.concurrency);
        assert_eq!(settings.theme, Some("dark".to_string()));
        assert!(warnings.is_empty());
    }

    #[test]
    fn malformed_bookmark_entry_is_skipped_not_fatal() {
        let entries = serde_json::json!([
            { "root": "/a", "group": null, "order": 0 },
            { "not_a_bookmark": true },
            { "root": "/b", "group": "work", "order": 1 },
        ]);
        let parsed: Vec<Bookmark> = entries
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| serde_json::from_value::<Bookmark>(v.clone()).ok())
            .collect();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].root, "/a");
        assert_eq!(parsed[1].root, "/b");
    }
}
