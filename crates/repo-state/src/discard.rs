use std::path::{Path, PathBuf};

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

fn path_args(command: &[&str], paths: &[PathBuf]) -> Vec<String> {
    let mut args: Vec<String> = command.iter().map(|s| s.to_string()).collect();
    args.push("--".to_string());
    args.extend(paths.iter().map(|p| p.to_string_lossy().into_owned()));
    args
}

pub async fn discard_tracked_paths(
    layer: &ProcessLayer,
    root: &Path,
    paths: &[PathBuf],
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no paths selected".to_string());
    }
    let result = layer
        .run(
            GitCall::new(root, path_args(&["restore", "--worktree"], paths)),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git restore failed: {}", result.stderr.trim()));
    }
    Ok(())
}

pub async fn delete_untracked_paths(
    layer: &ProcessLayer,
    root: &Path,
    paths: &[PathBuf],
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no paths selected".to_string());
    }
    let result = layer
        .run(
            GitCall::new(root, path_args(&["clean", "-f", "-d"], paths)),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git clean failed: {}", result.stderr.trim()));
    }
    Ok(())
}

pub async fn stash_paths(
    layer: &ProcessLayer,
    root: &Path,
    paths: &[PathBuf],
    message: Option<&str>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no paths selected".to_string());
    }
    let mut args = vec!["stash".to_string(), "push".to_string(), "-u".to_string()];
    if let Some(m) = message {
        args.push("-m".to_string());
        args.push(m.to_string());
    }
    args.push("--".to_string());
    args.extend(paths.iter().map(|p| p.to_string_lossy().into_owned()));
    let result = layer
        .run(
            GitCall::new(root, args),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git stash push failed: {}", result.stderr.trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::working_copy::query_working_copy_status;
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
        std::fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        dir
    }

    #[tokio::test]
    async fn discarding_a_tracked_edit_restores_the_committed_content() {
        let dir = init_repo();
        std::fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        discard_tracked_paths(&layer, dir.path(), &[PathBuf::from("tracked.txt")])
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("tracked.txt")).unwrap(),
            "base\n"
        );
    }

    #[tokio::test]
    async fn discarding_a_tracked_edit_leaves_a_staged_change_on_another_file_alone() {
        let dir = init_repo();
        std::fs::write(dir.path().join("other.txt"), "new").unwrap();
        Command::new("git")
            .args(["add", "other.txt"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        std::fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        discard_tracked_paths(&layer, dir.path(), &[PathBuf::from("tracked.txt")])
            .await
            .unwrap();

        let status = query_working_copy_status(&layer, dir.path())
            .await
            .into_known()
            .unwrap();
        assert!(status
            .changed
            .iter()
            .any(|e| e.path.as_path() == Path::new("other.txt")));
    }

    #[tokio::test]
    async fn deleting_an_untracked_file_removes_it_from_disk() {
        let dir = init_repo();
        std::fs::write(dir.path().join("scratch.txt"), "x").unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        delete_untracked_paths(&layer, dir.path(), &[PathBuf::from("scratch.txt")])
            .await
            .unwrap();

        assert!(!dir.path().join("scratch.txt").exists());
    }

    #[tokio::test]
    async fn stashing_a_path_removes_it_but_leaves_it_recoverable() {
        let dir = init_repo();
        std::fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        stash_paths(
            &layer,
            dir.path(),
            &[PathBuf::from("tracked.txt")],
            Some("gittree discard"),
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("tracked.txt")).unwrap(),
            "base\n"
        );

        Command::new("git")
            .args(["stash", "pop"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("tracked.txt")).unwrap(),
            "changed\n"
        );
    }

    #[tokio::test]
    async fn an_empty_path_list_is_refused_for_every_operation() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        assert!(discard_tracked_paths(&layer, dir.path(), &[])
            .await
            .is_err());
        assert!(delete_untracked_paths(&layer, dir.path(), &[])
            .await
            .is_err());
        assert!(stash_paths(&layer, dir.path(), &[], None).await.is_err());
    }
}
