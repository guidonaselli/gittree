use std::path::Path;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

use crate::commit::CommitOptions;
use crate::upstream::{resolve_upstream_basis, UpstreamBasis};

/// True when HEAD is already reachable from its upstream, meaning amending
/// it rewrites history other clones may have already fetched.
pub async fn head_is_published(layer: &ProcessLayer, root: &Path) -> Result<bool, String> {
    let branch_result = layer
        .run(
            GitCall::new(root, ["symbolic-ref", "--short", "-q", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    let current_branch = branch_result
        .ok()
        .then(|| branch_result.stdout_utf8_lossy().trim().to_string())
        .filter(|s| !s.is_empty());

    let basis = resolve_upstream_basis(layer, root, current_branch.as_deref()).await;
    let (UpstreamBasis::Configured { refname } | UpstreamBasis::Inferred { refname }) = basis
    else {
        return Ok(false);
    };

    let result = layer
        .run(
            GitCall::new(root, ["merge-base", "--is-ancestor", "HEAD", &refname]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.ok())
}

pub async fn amend(
    layer: &ProcessLayer,
    root: &Path,
    message: &str,
    options: &CommitOptions,
) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("commit message is empty".to_string());
    }
    let mut args = vec![
        "commit".to_string(),
        "--amend".to_string(),
        "-m".to_string(),
        message.to_string(),
    ];
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
    if !result.ok() {
        return Err(format!(
            "git commit --amend failed: {}",
            result.stderr.trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str], cwd: &Path| {
            let status = Command::new("git")
                .args(args)
                .current_dir(cwd)
                .status()
                .unwrap();
            assert!(status.success());
        };
        run(&["init", "-q", "-b", "main"], dir.path());
        run(&["config", "user.email", "test@example.com"], dir.path());
        run(&["config", "user.name", "Test"], dir.path());
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

    fn commit_file(dir: &Path, name: &str, contents: &str, message: &str) {
        std::fs::write(dir.join(name), contents).unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir)
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", message])
            .current_dir(dir)
            .status()
            .unwrap();
    }

    #[tokio::test]
    async fn amend_replaces_the_head_message_without_creating_a_new_commit() {
        let dir = init_repo();
        commit_file(dir.path(), "f.txt", "x", "original");
        let before_count = Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        amend(
            &layer,
            dir.path(),
            "amended message",
            &CommitOptions::default(),
        )
        .await
        .unwrap();

        assert_eq!(head_message(dir.path()), "amended message");
        let after_count = Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(before_count.stdout, after_count.stdout);
    }

    #[tokio::test]
    async fn head_is_published_is_false_with_no_upstream() {
        let dir = init_repo();
        commit_file(dir.path(), "f.txt", "x", "only commit");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let published = head_is_published(&layer, dir.path()).await.unwrap();
        assert!(!published);
    }

    #[tokio::test]
    async fn head_is_published_is_true_once_the_upstream_contains_head() {
        let remote_dir = init_repo();
        commit_file(remote_dir.path(), "f.txt", "x", "shared commit");

        let local_dir = tempfile::tempdir().unwrap();
        let status = Command::new("git")
            .args([
                "clone",
                "-q",
                remote_dir.path().to_str().unwrap(),
                local_dir.path().to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(status.success());
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(local_dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(local_dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let published = head_is_published(&layer, local_dir.path()).await.unwrap();
        assert!(
            published,
            "HEAD was just cloned from origin, so it must be published"
        );
    }

    #[tokio::test]
    async fn head_is_published_is_false_once_a_new_local_commit_is_ahead_of_upstream() {
        let remote_dir = init_repo();
        commit_file(remote_dir.path(), "f.txt", "x", "shared commit");

        let local_dir = tempfile::tempdir().unwrap();
        Command::new("git")
            .args([
                "clone",
                "-q",
                remote_dir.path().to_str().unwrap(),
                local_dir.path().to_str().unwrap(),
            ])
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(local_dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(local_dir.path())
            .status()
            .unwrap();
        commit_file(local_dir.path(), "g.txt", "y", "local-only commit");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let published = head_is_published(&layer, local_dir.path()).await.unwrap();
        assert!(!published, "the new local commit was never pushed");
    }
}
