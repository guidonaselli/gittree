use std::path::{Path, PathBuf};

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenOutcome {
    Repository { root: PathBuf },
    BareRepository { root: PathBuf },
    NotARepository,
}

pub async fn resolve_open(layer: &ProcessLayer, path: &Path) -> Result<OpenOutcome, String> {
    let bare_check = layer
        .run(
            GitCall::new(path, ["rev-parse", "--is-bare-repository"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;

    if !bare_check.ok() {
        return Ok(OpenOutcome::NotARepository);
    }

    if bare_check.stdout_utf8_lossy().trim() == "true" {
        let git_dir = layer
            .run(
                GitCall::new(path, ["rev-parse", "--absolute-git-dir"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .map_err(|e| e.to_string())?;
        if !git_dir.ok() {
            return Err(format!(
                "could not resolve the bare repository's git dir: {}",
                git_dir.stderr.trim()
            ));
        }
        return Ok(OpenOutcome::BareRepository {
            root: PathBuf::from(git_dir.stdout_utf8_lossy().trim()),
        });
    }

    let toplevel = layer
        .run(
            GitCall::new(path, ["rev-parse", "--show-toplevel"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !toplevel.ok() {
        return Err(format!(
            "could not resolve the repository root: {}",
            toplevel.stderr.trim()
        ));
    }
    Ok(OpenOutcome::Repository {
        root: PathBuf::from(toplevel.stdout_utf8_lossy().trim()),
    })
}

pub async fn init_repository(layer: &ProcessLayer, path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    let result = layer
        .run(
            GitCall::new(path, ["init", "-q"]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git init failed: {}", result.stderr.trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }

    fn init_repo(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "test@example.com"]);
        git(dir, &["config", "user.name", "Test"]);
        std::fs::write(dir.join("f.txt"), "hello").unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-q", "-m", "init"]);
    }

    #[tokio::test]
    async fn resolves_a_normal_working_tree() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let outcome = resolve_open(&layer, dir.path()).await.unwrap();
        assert_eq!(
            outcome,
            OpenOutcome::Repository {
                root: dir.path().canonicalize().unwrap()
            }
        );
    }

    #[tokio::test]
    async fn resolves_a_subdirectory_to_the_repository_root() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let outcome = resolve_open(&layer, &nested).await.unwrap();
        assert_eq!(
            outcome,
            OpenOutcome::Repository {
                root: dir.path().canonicalize().unwrap()
            }
        );
    }

    #[tokio::test]
    async fn resolves_a_bare_repository_distinctly() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "--bare"]);
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let outcome = resolve_open(&layer, dir.path()).await.unwrap();
        assert_eq!(
            outcome,
            OpenOutcome::BareRepository {
                root: dir.path().canonicalize().unwrap()
            }
        );
    }

    #[tokio::test]
    async fn resolves_a_linked_worktree_to_its_own_root() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        git(dir.path(), &["branch", "feature"]);
        let worktree_dir = dir.path().parent().unwrap().join(format!(
            "wt-{}",
            dir.path().file_name().unwrap().to_string_lossy()
        ));
        git(
            dir.path(),
            &["worktree", "add", worktree_dir.to_str().unwrap(), "feature"],
        );

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let outcome = resolve_open(&layer, &worktree_dir).await.unwrap();
        assert_eq!(
            outcome,
            OpenOutcome::Repository {
                root: worktree_dir.canonicalize().unwrap()
            }
        );

        git(
            dir.path(),
            &[
                "worktree",
                "remove",
                worktree_dir.to_str().unwrap(),
                "--force",
            ],
        );
    }

    #[tokio::test]
    async fn reports_a_non_repository_distinctly() {
        let dir = tempfile::tempdir().unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let outcome = resolve_open(&layer, dir.path()).await.unwrap();
        assert_eq!(outcome, OpenOutcome::NotARepository);
    }

    #[tokio::test]
    async fn init_repository_creates_a_real_repository() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("new-repo");
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        init_repository(&layer, &target).await.unwrap();
        let outcome = resolve_open(&layer, &target).await.unwrap();
        assert_eq!(
            outcome,
            OpenOutcome::Repository {
                root: target.canonicalize().unwrap()
            }
        );
    }
}
