use std::fs;
use std::path::{Path, PathBuf};

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IgnoreTarget {
    GitIgnore,
    GitInfoExclude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IgnorePatternKind {
    ExactPath,
    Extension,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IgnoreExplanation {
    pub source: String,
    pub line_number: u32,
    pub pattern: String,
    pub path: String,
}

/// Computes the suggested pattern for a given path and kind.
pub fn compute_ignore_pattern(path: &str, kind: IgnorePatternKind) -> Option<String> {
    let p = Path::new(path);
    match kind {
        IgnorePatternKind::ExactPath => Some(path.to_string()),
        IgnorePatternKind::Extension => {
            let ext = p.extension()?.to_str()?;
            Some(format!("*.{ext}"))
        }
        IgnorePatternKind::Directory => {
            let parent = p.parent()?;
            let parent_str = parent.to_str()?;
            if parent_str.is_empty() {
                None
            } else {
                Some(format!("{parent_str}/"))
            }
        }
    }
}

/// Queries `git check-ignore -v` to explain which rule ignores a path, if any.
pub async fn check_ignore(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
) -> Result<Option<IgnoreExplanation>, String> {
    let call = GitCall::new(root, ["check-ignore", "-v", "--", path]);
    let res = layer
        .run(call, Intent::Read, CancellationToken::new())
        .await
        .map_err(|e| e.to_string())?;

    if !res.ok() {
        if res.status == 1 {
            return Ok(None);
        }
        return Err(format!("check-ignore failed: {}", res.stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&res.stdout);
    let line = stdout.lines().next().unwrap_or("");
    if line.is_empty() {
        return Ok(None);
    }

    let mut tab_parts = line.split('\t');
    let meta = tab_parts.next().unwrap_or("");
    let matched_path = tab_parts.next().unwrap_or(path);

    let mut colon_parts = meta.splitn(3, ':');
    let source = colon_parts.next().unwrap_or("").to_string();
    let lineno: u32 = colon_parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let pattern = colon_parts.next().unwrap_or("").to_string();

    Ok(Some(IgnoreExplanation {
        source,
        line_number: lineno,
        pattern,
        path: matched_path.to_string(),
    }))
}

/// Resolves the absolute path to the target file (.gitignore or .git/info/exclude).
async fn resolve_target_file(
    layer: &ProcessLayer,
    root: &Path,
    target: IgnoreTarget,
) -> Result<PathBuf, String> {
    match target {
        IgnoreTarget::GitIgnore => Ok(root.join(".gitignore")),
        IgnoreTarget::GitInfoExclude => {
            let call = GitCall::new(root, ["rev-parse", "--git-path", "info/exclude"]);
            let res = layer
                .run(call, Intent::Read, CancellationToken::new())
                .await
                .map_err(|e| e.to_string())?;
            if !res.ok() {
                return Err(format!(
                    "failed to resolve info/exclude: {}",
                    res.stderr.trim()
                ));
            }
            let raw_path = String::from_utf8_lossy(&res.stdout).trim().to_string();
            let p = PathBuf::from(&raw_path);
            if p.is_absolute() {
                Ok(p)
            } else {
                Ok(root.join(p))
            }
        }
    }
}

/// Appends an ignore pattern to `.gitignore` or `.git/info/exclude`.
pub async fn add_ignore_rule(
    layer: &ProcessLayer,
    root: &Path,
    target: IgnoreTarget,
    pattern: &str,
) -> Result<(), String> {
    let file_path = resolve_target_file(layer, root, target).await?;

    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create dir {}: {e}", parent.display()))?;
    }

    let existing = if file_path.exists() {
        fs::read_to_string(&file_path)
            .map_err(|e| format!("failed to read {}: {e}", file_path.display()))?
    } else {
        String::new()
    };

    // Check if pattern is already present
    for line in existing.lines() {
        if line.trim() == pattern.trim() {
            return Ok(());
        }
    }

    let mut new_content = existing;
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str(pattern.trim());
    new_content.push('\n');

    fs::write(&file_path, new_content)
        .map_err(|e| format!("failed to write {}: {e}", file_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    fn init_repo() -> tempfile::TempDir {
        let td = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(td.path())
                .status()
                .unwrap();
            assert!(status.success());
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        td
    }

    #[test]
    fn compute_pattern_kinds() {
        assert_eq!(
            compute_ignore_pattern("src/app/main.rs", IgnorePatternKind::ExactPath),
            Some("src/app/main.rs".to_string())
        );
        assert_eq!(
            compute_ignore_pattern("src/app/main.rs", IgnorePatternKind::Extension),
            Some("*.rs".to_string())
        );
        assert_eq!(
            compute_ignore_pattern("src/app/main.rs", IgnorePatternKind::Directory),
            Some("src/app/".to_string())
        );
        assert_eq!(
            compute_ignore_pattern("root.txt", IgnorePatternKind::Directory),
            None
        );
    }

    #[tokio::test]
    async fn check_ignore_and_add_rule_round_trip() {
        let td = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Before adding rule, test.log is not ignored
        let expl = check_ignore(&layer, td.path(), "test.log").await.unwrap();
        assert_eq!(expl, None);

        // Add *.log to .gitignore
        add_ignore_rule(&layer, td.path(), IgnoreTarget::GitIgnore, "*.log")
            .await
            .unwrap();

        // Now test.log is ignored by .gitignore line 1
        let expl = check_ignore(&layer, td.path(), "test.log").await.unwrap();
        assert!(expl.is_some());
        let expl = expl.unwrap();
        assert_eq!(expl.source, ".gitignore");
        assert_eq!(expl.line_number, 1);
        assert_eq!(expl.pattern, "*.log");

        // Add local rule to .git/info/exclude
        add_ignore_rule(
            &layer,
            td.path(),
            IgnoreTarget::GitInfoExclude,
            "secret.key",
        )
        .await
        .unwrap();

        let expl_secret = check_ignore(&layer, td.path(), "secret.key")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(expl_secret.pattern, "secret.key");
        assert!(expl_secret.source.contains("exclude"));
    }
}
