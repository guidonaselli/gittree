use std::path::Path;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct CommitOptions {
    pub author: Option<String>,
    pub sign_off: bool,
    pub sign: bool,
}

/// Combines stdout and stderr verbatim, in that order.
pub(crate) fn verbatim_output(result: &git_process::GitResult) -> String {
    let stdout = result.stdout_utf8_lossy();
    let stderr = result.stderr.as_str();
    match (stdout.trim().is_empty(), stderr.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout.into_owned(),
        (true, false) => stderr.to_string(),
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

pub async fn commit(
    layer: &ProcessLayer,
    root: &Path,
    message: &str,
    options: &CommitOptions,
) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("commit message is empty".to_string());
    }
    let mut args = vec!["commit".to_string(), "-m".to_string(), message.to_string()];
    if let Some(author) = &options.author {
        args.push(format!("--author={author}"));
    }
    if options.sign_off {
        args.push("--signoff".to_string());
    }
    args.push(if options.sign {
        "--gpg-sign".to_string()
    } else {
        "--no-gpg-sign".to_string()
    });

    let result = layer
        .run(
            GitCall::new(root, args),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    let output = verbatim_output(&result);
    if !result.ok() {
        return Err(output);
    }
    Ok(output)
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

    fn head_message(dir: &Path) -> String {
        let out = Command::new("git")
            .args(["log", "-1", "--pretty=%B"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn head_sha(dir: &Path) -> String {
        let out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[tokio::test]
    async fn a_basic_commit_advances_head_with_the_given_message() {
        let dir = init_repo();
        std::fs::write(dir.path().join("f.txt"), "x").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        commit(
            &layer,
            dir.path(),
            "first commit",
            &CommitOptions::default(),
        )
        .await
        .unwrap();

        assert_eq!(head_message(dir.path()), "first commit");
    }

    #[tokio::test]
    async fn an_empty_message_is_refused_before_spawning_git() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let result = commit(&layer, dir.path(), "   ", &CommitOptions::default()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn an_author_override_is_reflected_in_the_commit() {
        let dir = init_repo();
        std::fs::write(dir.path().join("f.txt"), "x").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let options = CommitOptions {
            author: Some("Someone Else <someone@example.com>".to_string()),
            ..Default::default()
        };
        commit(&layer, dir.path(), "authored", &options)
            .await
            .unwrap();

        let out = Command::new("git")
            .args(["log", "-1", "--pretty=%an <%ae>"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "Someone Else <someone@example.com>"
        );
    }

    #[tokio::test]
    async fn sign_off_adds_the_trailer() {
        let dir = init_repo();
        std::fs::write(dir.path().join("f.txt"), "x").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let options = CommitOptions {
            sign_off: true,
            ..Default::default()
        };
        commit(&layer, dir.path(), "signed off", &options)
            .await
            .unwrap();

        assert!(head_message(dir.path()).contains("Signed-off-by: Test <test@example.com>"));
    }

    #[tokio::test]
    async fn a_signing_failure_fails_the_commit_rather_than_committing_unsigned() {
        let dir = init_repo();
        Command::new("git")
            .args(["config", "gpg.program", "/nonexistent-gpg-binary"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        std::fs::write(dir.path().join("f.txt"), "x").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        let before = head_sha(dir.path());

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let options = CommitOptions {
            sign: true,
            ..Default::default()
        };
        let result = commit(&layer, dir.path(), "should fail signed", &options).await;

        assert!(result.is_err());
        assert_eq!(
            head_sha(dir.path()),
            before,
            "HEAD must not advance on a failed signature"
        );
    }

    fn install_hook(dir: &Path, name: &str, script: &str) {
        let hooks_dir = dir.join(".git/hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        let path = hooks_dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn staged_paths(dir: &Path) -> String {
        let out = Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[tokio::test]
    async fn a_passing_hooks_output_is_surfaced_verbatim() {
        let dir = init_repo();
        install_hook(dir.path(), "pre-commit", "echo HOOK_MARKER_OK");
        std::fs::write(dir.path().join("f.txt"), "x").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let output = commit(
            &layer,
            dir.path(),
            "with a passing hook",
            &CommitOptions::default(),
        )
        .await
        .unwrap();

        assert!(
            output.contains("HOOK_MARKER_OK"),
            "hook stdout must be surfaced verbatim, got: {output}"
        );
    }

    #[tokio::test]
    async fn a_failing_precommit_hook_blocks_the_commit_and_leaves_the_index_untouched() {
        let dir = init_repo();
        commit_helper_first_commit(dir.path());
        install_hook(
            dir.path(),
            "pre-commit",
            "echo HOOK_MARKER_REJECTED >&2\nexit 1",
        );
        std::fs::write(dir.path().join("f.txt"), "changed").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        let before = head_sha(dir.path());

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let result = commit(
            &layer,
            dir.path(),
            "should be rejected",
            &CommitOptions::default(),
        )
        .await;

        let err = result.expect_err("a failing pre-commit hook must fail the commit");
        assert!(
            err.contains("HOOK_MARKER_REJECTED"),
            "the hook's own diagnostic must be surfaced verbatim, got: {err}"
        );
        assert_eq!(head_sha(dir.path()), before, "HEAD must not advance");
        assert_eq!(
            staged_paths(dir.path()),
            "f.txt",
            "the staged change must remain staged, not be rolled back"
        );
    }

    fn commit_helper_first_commit(dir: &Path) {
        std::fs::write(dir.join("f.txt"), "initial").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir)
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", "seed"])
            .current_dir(dir)
            .status()
            .unwrap();
    }
}
