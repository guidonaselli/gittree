use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
struct RawUserTheme {
    name: Option<String>,
    mode: Option<String>,
    #[serde(default)]
    tokens: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserThemeInfo {
    pub id: String,
    pub name: String,
    pub mode: String, // "light" or "dark"
    pub path: String,
    pub tokens: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserThemeParseError {
    pub file: String,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserThemesResult {
    pub themes_dir: String,
    pub themes: Vec<UserThemeInfo>,
    pub errors: Vec<UserThemeParseError>,
}

pub fn default_user_themes_dir() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        });
    config_dir.join("gittree").join("themes")
}

pub fn parse_user_theme(path: &Path, content: &str) -> Result<UserThemeInfo, UserThemeParseError> {
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let id = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| file_name.clone());

    let raw: RawUserTheme = match toml::from_str(content) {
        Ok(r) => r,
        Err(err) => {
            let line = err.span().map(|span| {
                content[..span.start.min(content.len())]
                    .chars()
                    .filter(|&c| c == '\n')
                    .count()
                    + 1
            });
            return Err(UserThemeParseError {
                file: file_name,
                line,
                message: err.message().to_string(),
            });
        }
    };

    let name = match raw.name {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => {
            return Err(UserThemeParseError {
                file: file_name,
                line: None,
                message: "Missing or empty required field 'name'".to_string(),
            });
        }
    };

    let mode = match raw.mode {
        Some(ref m) if m == "light" || m == "dark" => m.clone(),
        Some(other) => {
            return Err(UserThemeParseError {
                file: file_name,
                line: None,
                message: format!("Invalid 'mode': expected 'light' or 'dark', got '{other}'"),
            });
        }
        None => {
            return Err(UserThemeParseError {
                file: file_name,
                line: None,
                message: "Missing required field 'mode' ('light' or 'dark')".to_string(),
            });
        }
    };

    // Filter tokens to ensure they are valid CSS variables
    let mut validated_tokens = BTreeMap::new();
    for (k, v) in raw.tokens {
        let trimmed_k = k.trim();
        let trimmed_v = v.trim();
        if !trimmed_k.starts_with("--") {
            return Err(UserThemeParseError {
                file: file_name,
                line: None,
                message: format!(
                    "Token '{trimmed_k}' is invalid: token names must start with '--'"
                ),
            });
        }
        validated_tokens.insert(trimmed_k.to_string(), trimmed_v.to_string());
    }

    Ok(UserThemeInfo {
        id,
        name,
        mode,
        path: path.to_string_lossy().to_string(),
        tokens: validated_tokens,
    })
}

pub fn load_user_themes(dir: &Path) -> UserThemesResult {
    let mut themes = Vec::new();
    let mut errors = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
                match std::fs::read_to_string(&path) {
                    Ok(content) => match parse_user_theme(&path, &content) {
                        Ok(theme) => themes.push(theme),
                        Err(err) => errors.push(err),
                    },
                    Err(e) => {
                        errors.push(UserThemeParseError {
                            file: path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default(),
                            line: None,
                            message: format!("Failed to read file: {e}"),
                        });
                    }
                }
            }
        }
    }

    themes.sort_by(|a, b| a.name.cmp(&b.name));
    errors.sort_by(|a, b| a.file.cmp(&b.file));

    UserThemesResult {
        themes_dir: dir.to_string_lossy().to_string(),
        themes,
        errors,
    }
}

pub fn load_available_themes(user_dir: &Path) -> UserThemesResult {
    let mut map: BTreeMap<String, UserThemeInfo> = BTreeMap::new();
    let mut errors = Vec::new();

    // Check system and repo-relative theme paths first
    for candidate in [
        PathBuf::from("/usr/share/gittree/themes"),
        PathBuf::from("themes"),
        PathBuf::from("../themes"),
    ] {
        if candidate.is_dir() {
            let res = load_user_themes(&candidate);
            for t in res.themes {
                map.insert(t.id.clone(), t);
            }
            errors.extend(res.errors);
        }
    }

    // User themes override and augment system themes
    if user_dir.is_dir() {
        let user_res = load_user_themes(user_dir);
        for t in user_res.themes {
            map.insert(t.id.clone(), t);
        }
        errors.extend(user_res.errors);
    }

    let mut themes: Vec<UserThemeInfo> = map.into_values().collect();
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    errors.sort_by(|a, b| a.file.cmp(&b.file));

    UserThemesResult {
        themes_dir: user_dir.to_string_lossy().to_string(),
        themes,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_user_theme_with_partial_tokens() {
        let toml = r##"
name = "Solarized Dark"
mode = "dark"

[tokens]
--color-bg = "#002b36"
--color-text = "#839496"
--color-accent = "#268bd2"
"##;
        let path = Path::new("/test/themes/solarized-dark.toml");
        let theme = parse_user_theme(path, toml).expect("valid theme must parse");
        assert_eq!(theme.id, "solarized-dark");
        assert_eq!(theme.name, "Solarized Dark");
        assert_eq!(theme.mode, "dark");
        assert_eq!(theme.tokens.get("--color-bg").unwrap(), "#002b36");
        assert_eq!(theme.tokens.get("--color-text").unwrap(), "#839496");
        assert_eq!(theme.tokens.get("--color-accent").unwrap(), "#268bd2");
    }

    #[test]
    fn reports_syntax_error_with_line_number() {
        let malformed = r#"
name = "Broken Theme"
mode = "light"
invalid toml line here without equals
"#;
        let path = Path::new("/test/themes/broken.toml");
        let err = parse_user_theme(path, malformed).expect_err("syntax error must fail");
        assert_eq!(err.file, "broken.toml");
        assert_eq!(err.line, Some(4));
        assert!(!err.message.is_empty());
    }

    #[test]
    fn reports_missing_name_or_invalid_mode() {
        let no_name = r##"
mode = "dark"
[tokens]
--color-bg = "#111111"
"##;
        let err1 = parse_user_theme(Path::new("t1.toml"), no_name).unwrap_err();
        assert!(err1.message.contains("name"));

        let bad_mode = r#"
name = "Test"
mode = "neon"
"#;
        let err2 = parse_user_theme(Path::new("t2.toml"), bad_mode).unwrap_err();
        assert!(err2.message.contains("mode"));
    }

    #[test]
    fn reports_invalid_token_prefix() {
        let bad_token = r##"
name = "Test"
mode = "light"
[tokens]
background = "#ffffff"
"##;
        let err = parse_user_theme(Path::new("t3.toml"), bad_token).unwrap_err();
        assert!(err.message.contains("must start with '--'"));
    }

    #[test]
    fn loads_themes_and_separates_valid_and_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let valid_path = tmp.path().join("nord.toml");
        std::fs::write(
            &valid_path,
            r##"
name = "Nord"
mode = "dark"
[tokens]
--color-bg = "#2e3440"
"##,
        )
        .unwrap();

        let invalid_path = tmp.path().join("broken.toml");
        std::fs::write(&invalid_path, "not a valid toml = [").unwrap();

        let result = load_user_themes(tmp.path());
        assert_eq!(result.themes.len(), 1);
        assert_eq!(result.themes[0].name, "Nord");
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].file, "broken.toml");
    }

    #[test]
    fn load_available_themes_finds_repo_themes() {
        let empty_user_dir = tempfile::tempdir().unwrap();
        let result = load_available_themes(empty_user_dir.path());
        assert!(result.themes.len() >= 6);
        let names: Vec<_> = result.themes.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Catppuccin Mocha"));
        assert!(names.contains(&"Nord"));
        assert!(names.contains(&"Tokyo Night"));
    }
}
