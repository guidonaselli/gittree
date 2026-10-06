use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
struct RawPalette {
    mode: Option<String>,
    accent: Option<String>,
    background: Option<String>,
    dark_background: Option<String>,
    lighter_background: Option<String>,
    foreground: Option<String>,
    dark_foreground: Option<String>,
    light_foreground: Option<String>,
    red: Option<String>,
    green: Option<String>,
    yellow: Option<String>,
    blue: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DesktopPalette {
    pub mode: String,
    pub tokens: BTreeMap<String, String>,
}

fn published_theme_path() -> PathBuf {
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/state")
        });
    state_dir.join("omarchy/current/theme/colors.toml")
}

pub fn published_theme_watch_dir() -> PathBuf {
    published_theme_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(published_theme_path)
}

fn map_to_tokens(raw: &RawPalette) -> BTreeMap<String, String> {
    let mut tokens = BTreeMap::new();
    let mut set = |key: &str, value: &Option<String>| {
        if let Some(v) = value {
            tokens.insert(key.to_string(), v.clone());
        }
    };
    set("--color-accent", &raw.accent);
    set("--color-focus-ring", &raw.accent);
    set("--color-bg", &raw.background);
    set("--color-bg-inset", &raw.dark_background);
    set("--color-bg-subtle", &raw.lighter_background);
    set("--color-text", &raw.foreground);
    set("--color-text-muted", &raw.light_foreground);
    set("--color-text-faint", &raw.dark_foreground);
    set("--color-success", &raw.green);
    set("--color-danger", &raw.red);
    set("--color-warning", &raw.yellow);
    set("--color-info", &raw.blue);
    tokens
}

pub fn read_published_palette() -> Result<Option<DesktopPalette>, String> {
    parse_palette_file(&published_theme_path())
}

fn parse_palette_file(path: &std::path::Path) -> Result<Option<DesktopPalette>, String> {
    let raw_text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return Ok(None),
    };
    let raw: RawPalette =
        toml::from_str(&raw_text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mode = raw.mode.clone().unwrap_or_else(|| "dark".to_string());
    Ok(Some(DesktopPalette {
        mode,
        tokens: map_to_tokens(&raw),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"
mode = "dark"

accent = "#82FB9C"
selection = "#1f253a"

background = "#0B0C16"
dark_background = "#080910"
lighter_background = "#151828"

foreground = "#ddf7ff"
dark_foreground = "#6a6e95"
light_foreground = "#b5c5db"

red = "#50f872"
green = "#4fe88f"
yellow = "#50f7d4"
blue = "#829dd4"
"##;

    #[test]
    fn parses_a_real_shaped_palette_and_maps_known_roles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("colors.toml");
        std::fs::write(&path, SAMPLE).unwrap();

        let palette = parse_palette_file(&path).unwrap().unwrap();
        assert_eq!(palette.mode, "dark");
        assert_eq!(
            palette.tokens.get("--color-accent"),
            Some(&"#82FB9C".to_string())
        );
        assert_eq!(
            palette.tokens.get("--color-bg"),
            Some(&"#0B0C16".to_string())
        );
        assert_eq!(
            palette.tokens.get("--color-text"),
            Some(&"#ddf7ff".to_string())
        );
        assert_eq!(
            palette.tokens.get("--color-success"),
            Some(&"#4fe88f".to_string())
        );
        assert_eq!(
            palette.tokens.get("--color-danger"),
            Some(&"#50f872".to_string())
        );
    }

    #[test]
    fn unknown_fields_are_ignored_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("colors.toml");
        std::fs::write(
            &path,
            format!("{SAMPLE}\nhyprland_active_border = \"rgba(...)\"\n"),
        )
        .unwrap();
        assert!(parse_palette_file(&path).unwrap().is_some());
    }

    #[test]
    fn missing_file_resolves_to_none_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = parse_palette_file(&dir.path().join("does-not-exist.toml")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn malformed_file_is_a_named_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("colors.toml");
        std::fs::write(&path, "this is not valid toml {{{").unwrap();
        let err = parse_palette_file(&path).unwrap_err();
        assert!(err.contains("colors.toml"));
    }

    #[test]
    fn missing_mode_defaults_to_dark() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("colors.toml");
        std::fs::write(&path, "accent = \"#ffffff\"\n").unwrap();
        let palette = parse_palette_file(&path).unwrap().unwrap();
        assert_eq!(palette.mode, "dark");
    }

    #[test]
    fn parses_the_real_published_theme_on_this_machine() {
        let palette = read_published_palette().unwrap();
        let Some(palette) = palette else {
            return;
        };
        assert!(!palette.tokens.is_empty());
        println!("mode={} tokens={:?}", palette.mode, palette.tokens);
    }
}
