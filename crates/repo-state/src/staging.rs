use std::path::{Path, PathBuf};

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

fn path_args(command: &[&str], paths: &[PathBuf]) -> Vec<String> {
    let mut args: Vec<String> = command.iter().map(|s| s.to_string()).collect();
    args.push("--".to_string());
    args.extend(paths.iter().map(|p| p.to_string_lossy().into_owned()));
    args
}

pub async fn stage_paths(
    layer: &ProcessLayer,
    root: &Path,
    paths: &[PathBuf],
) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let result = layer
        .run(
            GitCall::new(root, path_args(&["add", "-A"], paths)),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git add failed: {}", result.stderr.trim()));
    }
    Ok(())
}

pub async fn unstage_paths(
    layer: &ProcessLayer,
    root: &Path,
    paths: &[PathBuf],
) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let result = layer
        .run(
            GitCall::new(root, path_args(&["restore", "--staged"], paths)),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!(
            "git restore --staged failed: {}",
            result.stderr.trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::working_copy::{query_working_copy_status, ChangeCode};
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
        std::fs::write(dir.path().join("tracked.txt"), "base").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        dir
    }

    fn entry_codes(
        status: &crate::WorkingCopyStatus,
        path: &str,
    ) -> Option<(ChangeCode, ChangeCode)> {
        status
            .changed
            .iter()
            .find(|e| e.path.as_path() == Path::new(path))
            .map(|e| (e.staged, e.unstaged))
    }

    #[tokio::test]
    async fn staging_an_untracked_file_moves_it_to_staged() {
        let dir = init_repo();
        std::fs::write(dir.path().join("new.txt"), "content").unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        stage_paths(&layer, dir.path(), &[PathBuf::from("new.txt")])
            .await
            .unwrap();

        let status = query_working_copy_status(&layer, dir.path())
            .await
            .into_known()
            .unwrap();
        assert_eq!(
            entry_codes(&status, "new.txt"),
            Some((ChangeCode::Added, ChangeCode::Unmodified))
        );
        assert!(status.untracked.is_empty());
    }

    #[tokio::test]
    async fn unstaging_reverts_to_unstaged_without_discarding_the_edit() {
        let dir = init_repo();
        std::fs::write(dir.path().join("tracked.txt"), "changed").unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        stage_paths(&layer, dir.path(), &[PathBuf::from("tracked.txt")])
            .await
            .unwrap();

        unstage_paths(&layer, dir.path(), &[PathBuf::from("tracked.txt")])
            .await
            .unwrap();

        let status = query_working_copy_status(&layer, dir.path())
            .await
            .into_known()
            .unwrap();
        assert_eq!(
            entry_codes(&status, "tracked.txt"),
            Some((ChangeCode::Unmodified, ChangeCode::Modified))
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("tracked.txt")).unwrap(),
            "changed"
        );
    }

    #[tokio::test]
    async fn staging_a_deletion_records_it_as_a_staged_delete() {
        let dir = init_repo();
        std::fs::remove_file(dir.path().join("tracked.txt")).unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        stage_paths(&layer, dir.path(), &[PathBuf::from("tracked.txt")])
            .await
            .unwrap();

        let status = query_working_copy_status(&layer, dir.path())
            .await
            .into_known()
            .unwrap();
        assert_eq!(
            entry_codes(&status, "tracked.txt"),
            Some((ChangeCode::Deleted, ChangeCode::Unmodified))
        );
    }

    #[tokio::test]
    async fn staging_a_rename_covers_both_the_old_and_new_path() {
        let dir = init_repo();
        std::fs::write(
            dir.path().join("orig.txt"),
            "enough content to be detected as similar for a rename",
        )
        .unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", "add orig"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        std::fs::rename(dir.path().join("orig.txt"), dir.path().join("renamed.txt")).unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        stage_paths(
            &layer,
            dir.path(),
            &[PathBuf::from("orig.txt"), PathBuf::from("renamed.txt")],
        )
        .await
        .unwrap();

        let status = query_working_copy_status(&layer, dir.path())
            .await
            .into_known()
            .unwrap();
        let entry = status
            .changed
            .iter()
            .find(|e| e.path.as_path() == Path::new("renamed.txt"))
            .expect("expected the renamed entry");
        assert_eq!(entry.staged, ChangeCode::Renamed);
        assert_eq!(
            entry.rename_or_copy_from.as_ref().map(|(p, _)| p.clone()),
            Some(PathBuf::from("orig.txt"))
        );
    }

    #[tokio::test]
    async fn an_empty_path_list_is_a_no_op_not_a_stage_everything() {
        let dir = init_repo();
        std::fs::write(dir.path().join("untouched.txt"), "x").unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        stage_paths(&layer, dir.path(), &[]).await.unwrap();

        let status = query_working_copy_status(&layer, dir.path())
            .await
            .into_known()
            .unwrap();
        assert_eq!(status.untracked, vec![PathBuf::from("untouched.txt")]);
    }
}
