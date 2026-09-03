//! Settings and bookmarks persistence. Both live under the XDG config
//! directory as plain, hand-editable JSON, never inside a repository.
//! Settings validate per-key: one bad value falls back to its default
//! rather than rejecting the whole file.

use std::fs;
use std::path::{Path, PathBuf};
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
    load_settings_from(&settings_path())
}

fn load_settings_from(path: &Path) -> SettingsLoadResult {
    let default = Settings::default();
    let raw = match fs::read_to_string(path) {
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
    save_settings_to(&settings_path(), settings)
}

fn save_settings_to(path: &Path, settings: &Settings) -> std::io::Result<()> {
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    let text = serde_json::to_string_pretty(settings).expect("Settings serializes");
    fs::write(path, text)
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
    load_bookmarks_from(&bookmarks_path())
}

fn load_bookmarks_from(path: &Path) -> Vec<Bookmark> {
    let Ok(raw) = fs::read_to_string(path) else {
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
    save_bookmarks_to(&bookmarks_path(), bookmarks)
}

fn save_bookmarks_to(path: &Path, bookmarks: &[Bookmark]) -> std::io::Result<()> {
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    let text = serde_json::to_string_pretty(bookmarks).expect("Vec<Bookmark> serializes");
    fs::write(path, text)
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
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"theme":"dark"}"#).unwrap();

        let result = load_settings_from(&path);
        assert_eq!(result.settings.concurrency, Settings::default().concurrency);
        assert_eq!(result.settings.theme, Some("dark".to_string()));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn invalid_concurrency_falls_back_with_a_named_warning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"concurrency":-1,"theme":"light"}"#).unwrap();

        let result = load_settings_from(&path);
        assert_eq!(result.settings.concurrency, Settings::default().concurrency);
        assert_eq!(result.settings.theme, Some("light".to_string()));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("concurrency"));
    }

    #[test]
    fn missing_settings_file_uses_defaults_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_settings_from(&dir.path().join("does-not-exist.json"));
        assert_eq!(result.settings.concurrency, Settings::default().concurrency);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn settings_round_trip_through_the_real_save_and_load_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let settings = Settings {
            concurrency: 4,
            theme: Some("dark".to_string()),
        };

        save_settings_to(&path, &settings).unwrap();
        let loaded = load_settings_from(&path);

        assert_eq!(loaded.settings.concurrency, 4);
        assert_eq!(loaded.settings.theme, Some("dark".to_string()));
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn malformed_bookmark_entry_is_skipped_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bookmarks.json");
        std::fs::write(
            &path,
            r#"[{"root":"/a","group":null,"order":0},{"not_a_bookmark":true},{"root":"/b","group":"work","order":1}]"#,
        )
        .unwrap();

        let parsed = load_bookmarks_from(&path);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].root, "/a");
        assert_eq!(parsed[1].root, "/b");
    }

    #[test]
    fn missing_bookmarks_file_is_an_empty_list_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let parsed = load_bookmarks_from(&dir.path().join("does-not-exist.json"));
        assert!(parsed.is_empty());
    }

    #[test]
    fn bookmarks_round_trip_through_the_real_save_and_load_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bookmarks.json");
        let bookmarks = vec![
            Bookmark {
                root: "/a".to_string(),
                group: None,
                order: 0,
            },
            Bookmark {
                root: "/b".to_string(),
                group: Some("work".to_string()),
                order: 1,
            },
        ];

        save_bookmarks_to(&path, &bookmarks).unwrap();
        let loaded = load_bookmarks_from(&path);

        assert_eq!(loaded, bookmarks);
    }
}
