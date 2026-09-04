use std::path::Path;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CommitMessageTemplate {
    pub content: String,
    pub comment_char: String,
}

async fn read_config(layer: &ProcessLayer, root: &Path, key: &str) -> Option<String> {
    let result = layer
        .run(
            GitCall::new(root, ["config", "--get", key]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .ok()?;
    if !result.ok() {
        return None;
    }
    let value = result.stdout_utf8_lossy().trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn resolve_template_path(root: &Path, configured: &str) -> std::path::PathBuf {
    if let Some(rest) = configured.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    let path = std::path::PathBuf::from(configured);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

/// `commit.template`'s content plus `core.commentChar`.
pub async fn commit_message_template(
    layer: &ProcessLayer,
    root: &Path,
) -> Result<Option<CommitMessageTemplate>, String> {
    let Some(template_path) = read_config(layer, root, "commit.template").await else {
        return Ok(None);
    };
    let comment_char = read_config(layer, root, "core.commentChar")
        .await
        .filter(|c| c != "auto" && !c.is_empty())
        .unwrap_or_else(|| "#".to_string());

    let path = resolve_template_path(root, &template_path);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read commit.template at {}: {e}", path.display()))?;

    Ok(Some(CommitMessageTemplate {
        content,
        comment_char,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .status()
                .unwrap();
            assert!(status.success());
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        dir
    }

    #[tokio::test]
    async fn no_template_configured_resolves_to_none() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let template = commit_message_template(&layer, dir.path()).await.unwrap();
        assert!(template.is_none());
    }

    #[tokio::test]
    async fn a_configured_template_is_read_with_the_default_comment_char() {
        let dir = init_repo();
        std::fs::write(dir.path().join(".gitmessage"), "Subject\n\n# a hint\n").unwrap();
        Command::new("git")
            .args(["config", "commit.template", ".gitmessage"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let template = commit_message_template(&layer, dir.path())
            .await
            .unwrap()
            .expect("a template was configured");

        assert_eq!(template.content, "Subject\n\n# a hint\n");
        assert_eq!(template.comment_char, "#");
    }

    #[tokio::test]
    async fn a_custom_comment_char_is_reported() {
        let dir = init_repo();
        std::fs::write(dir.path().join(".gitmessage"), "Subject\n\n; a hint\n").unwrap();
        Command::new("git")
            .args(["config", "commit.template", ".gitmessage"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "core.commentChar", ";"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let template = commit_message_template(&layer, dir.path())
            .await
            .unwrap()
            .expect("a template was configured");

        assert_eq!(template.comment_char, ";");
    }

    #[tokio::test]
    async fn a_tilde_relative_template_path_is_expanded_against_home() {
        let dir = init_repo();
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join(".gitmessage"), "Home template\n").unwrap();
        Command::new("git")
            .args(["config", "commit.template", "~/.gitmessage"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let previous_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home.path());
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let template = commit_message_template(&layer, dir.path()).await;
        if let Some(previous_home) = previous_home {
            std::env::set_var("HOME", previous_home);
        }

        let template = template.unwrap().expect("a template was configured");
        assert_eq!(template.content, "Home template\n");
    }
}
