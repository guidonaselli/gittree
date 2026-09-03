use std::path::{Path, PathBuf};

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

use crate::resolved::Resolved;
use crate::upstream::{resolve_upstream_basis, UpstreamBasis};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum InProgressOperation {
    None,
    Merge,
    Rebase,
    RebaseInteractive,
    CherryPick,
    Revert,
    Bisect,
    AmSession,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Branch {
    Named(String),
    Detached { commit: String },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AheadBehind {
    pub basis: UpstreamBasis,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PathCounts {
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RepositoryState {
    pub root: PathBuf,
    pub branch: Resolved<Branch>,
    pub ahead_behind: Resolved<AheadBehind>,
    pub paths: Resolved<PathCounts>,
    pub in_progress: InProgressOperation,
    pub stash_count: Resolved<u32>,
    pub submodule_count: Resolved<u32>,
}

fn detect_in_progress(git_dir: &Path) -> InProgressOperation {
    if git_dir.join("MERGE_HEAD").exists() {
        InProgressOperation::Merge
    } else if git_dir.join("rebase-merge").is_dir() {
        InProgressOperation::RebaseInteractive
    } else if git_dir.join("rebase-apply").join("applying").exists() {
        InProgressOperation::AmSession
    } else if git_dir.join("rebase-apply").is_dir() {
        InProgressOperation::Rebase
    } else if git_dir.join("CHERRY_PICK_HEAD").exists() {
        InProgressOperation::CherryPick
    } else if git_dir.join("REVERT_HEAD").exists() {
        InProgressOperation::Revert
    } else if git_dir.join("BISECT_LOG").exists() {
        InProgressOperation::Bisect
    } else {
        InProgressOperation::None
    }
}

/// Parses `git status --porcelain=v2 -z`. NUL-delimited so paths with
/// newlines or spaces are never ambiguous.
fn parse_porcelain_v2(raw: &[u8]) -> PathCounts {
    let mut counts = PathCounts {
        staged: 0,
        unstaged: 0,
        untracked: 0,
        conflicted: 0,
    };
    let text = String::from_utf8_lossy(raw);
    for field in text.split('\0') {
        if field.is_empty() {
            continue;
        }
        let mut parts = field.splitn(2, ' ');
        let kind = parts.next().unwrap_or("");
        match kind {
            "1" | "2" => {
                // ordinary/renamed changed entry: XY at start of the 2nd token
                if let Some(rest) = parts.next() {
                    let xy = &rest[0..2.min(rest.len())];
                    let x = xy.chars().next().unwrap_or('.');
                    let y = xy.chars().nth(1).unwrap_or('.');
                    if x != '.' {
                        counts.staged += 1;
                    }
                    if y != '.' {
                        counts.unstaged += 1;
                    }
                }
            }
            "u" => counts.conflicted += 1,
            "?" => counts.untracked += 1,
            _ => {}
        }
    }
    counts
}

pub async fn query_repository_state(
    layer: &ProcessLayer,
    root: &Path,
    git_dir: &Path,
) -> RepositoryState {
    let branch = match layer
        .run(
            GitCall::new(root, ["symbolic-ref", "--short", "-q", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
    {
        Ok(r) if r.ok() => {
            let name = r.stdout_utf8_lossy().trim().to_string();
            Resolved::known(Branch::Named(name))
        }
        _ => match layer
            .run(
                GitCall::new(root, ["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
        {
            Ok(r) if r.ok() => Resolved::known(Branch::Detached {
                commit: r.stdout_utf8_lossy().trim().to_string(),
            }),
            Ok(r) => Resolved::unknown(format!("rev-parse HEAD failed: {}", r.stderr.trim())),
            Err(e) => Resolved::unknown(e.to_string()),
        },
    };

    let branch_name = branch.as_known().and_then(|b| match b {
        Branch::Named(n) => Some(n.as_str()),
        Branch::Detached { .. } => None,
    });

    let ahead_behind = if let Some(name) = branch_name {
        let basis = resolve_upstream_basis(layer, root, Some(name)).await;
        match basis.refname() {
            Some(refname) => {
                let range = format!("{refname}...HEAD");
                match layer
                    .run(
                        GitCall::new(root, ["rev-list", "--left-right", "--count", &range]),
                        Intent::Read,
                        CancellationToken::new(),
                    )
                    .await
                {
                    Ok(r) if r.ok() => {
                        let text = r.stdout_utf8_lossy();
                        let mut nums = text
                            .split_whitespace()
                            .filter_map(|s| s.parse::<u32>().ok());
                        let behind = nums.next().unwrap_or(0);
                        let ahead = nums.next().unwrap_or(0);
                        Resolved::known(AheadBehind {
                            basis,
                            ahead,
                            behind,
                        })
                    }
                    Ok(r) => Resolved::unknown(format!("rev-list failed: {}", r.stderr.trim())),
                    Err(e) => Resolved::unknown(e.to_string()),
                }
            }
            None => Resolved::unknown("no upstream and no same-named remote branch"),
        }
    } else {
        Resolved::unknown("HEAD is detached; no branch to compare")
    };

    let paths = match layer
        .run(
            GitCall::new(
                root,
                ["status", "--porcelain=v2", "-z", "--untracked-files=all"],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
    {
        Ok(r) if r.ok() => Resolved::known(parse_porcelain_v2(&r.stdout)),
        Ok(r) => Resolved::unknown(format!("status failed: {}", r.stderr.trim())),
        Err(e) => Resolved::unknown(e.to_string()),
    };

    let in_progress = detect_in_progress(git_dir);

    let stash_count = match layer
        .run(
            GitCall::new(root, ["stash", "list"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
    {
        Ok(r) if r.ok() => Resolved::known(
            r.stdout_utf8_lossy()
                .lines()
                .filter(|l| !l.is_empty())
                .count() as u32,
        ),
        Ok(r) => Resolved::unknown(format!("stash list failed: {}", r.stderr.trim())),
        Err(e) => Resolved::unknown(e.to_string()),
    };

    let submodule_count = match layer
        .run(
            GitCall::new(
                root,
                ["config", "-f", ".gitmodules", "--get-regexp", r"\.path$"],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
    {
        Ok(r) if r.ok() => Resolved::known(
            r.stdout_utf8_lossy()
                .lines()
                .filter(|l| !l.is_empty())
                .count() as u32,
        ),
        Ok(r) if r.status == 1 => Resolved::known(0), // no .gitmodules or no matches
        Ok(r) => Resolved::unknown(format!("reading .gitmodules failed: {}", r.stderr.trim())),
        Err(e) => Resolved::unknown(e.to_string()),
    };

    RepositoryState {
        root: root.to_path_buf(),
        branch,
        ahead_behind,
        paths,
        in_progress,
        stash_count,
        submodule_count,
    }
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
        std::fs::write(dir.path().join("f.txt"), "hello").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        dir
    }

    #[tokio::test]
    async fn clean_repo_reports_named_branch_and_zero_paths() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let state = query_repository_state(&layer, dir.path(), &dir.path().join(".git")).await;
        assert_eq!(
            state.branch,
            Resolved::known(Branch::Named("main".to_string()))
        );
        let paths = state.paths.as_known().unwrap();
        assert_eq!(
            (
                paths.staged,
                paths.unstaged,
                paths.untracked,
                paths.conflicted
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(state.in_progress, InProgressOperation::None);
        assert_eq!(state.submodule_count, Resolved::known(0));
    }

    #[tokio::test]
    async fn detached_head_is_reported_not_invented() {
        let dir = init_repo();
        let head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
        Command::new("git")
            .args(["checkout", "-q", &head])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let state = query_repository_state(&layer, dir.path(), &dir.path().join(".git")).await;
        match state.branch {
            Resolved::Known(Branch::Detached { commit }) => assert_eq!(commit, head),
            other => panic!("expected detached HEAD, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn untracked_and_staged_files_are_counted_distinctly() {
        let dir = init_repo();
        std::fs::write(dir.path().join("untracked.txt"), "x").unwrap();
        std::fs::write(dir.path().join("staged.txt"), "x").unwrap();
        Command::new("git")
            .args(["add", "staged.txt"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let state = query_repository_state(&layer, dir.path(), &dir.path().join(".git")).await;
        let paths = state.paths.as_known().unwrap();
        assert_eq!(paths.untracked, 1);
        assert_eq!(paths.staged, 1);
    }

    #[test]
    fn parses_porcelain_v2_conflict_marker() {
        let raw = b"u UU N... 100644 100644 100644 100644 aaa bbb ccc conflicted.txt\0";
        let counts = parse_porcelain_v2(raw);
        assert_eq!(counts.conflicted, 1);
    }

    #[tokio::test]
    async fn filename_with_a_space_counts_correctly() {
        let dir = init_repo();
        std::fs::write(dir.path().join("has space.txt"), "x").unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let state = query_repository_state(&layer, dir.path(), &dir.path().join(".git")).await;
        assert_eq!(state.paths.as_known().unwrap().untracked, 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn filename_with_a_literal_quote_counts_correctly() {
        let dir = init_repo();
        std::fs::write(dir.path().join("has\"quote.txt"), "x").unwrap();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let state = query_repository_state(&layer, dir.path(), &dir.path().join(".git")).await;
        assert_eq!(state.paths.as_known().unwrap().untracked, 1);
    }

    #[test]
    fn filename_with_an_embedded_newline_counts_correctly() {
        // `-z` disables path quoting: an embedded newline is a literal byte
        // in the field, so only the NUL terminator ends the record.
        let mut raw = b"? has\nnewline.txt".to_vec();
        raw.push(0);
        let counts = parse_porcelain_v2(&raw);
        assert_eq!(counts.untracked, 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn non_utf8_filename_counts_correctly_without_crashing() {
        use std::os::unix::ffi::OsStrExt;

        let dir = init_repo();
        let bytes = [b'b', b'a', 0xFFu8, b'd', b'.', b't', b'x', b't'];
        let name = std::ffi::OsStr::from_bytes(&bytes);
        std::fs::write(dir.path().join(name), "x").unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let state = query_repository_state(&layer, dir.path(), &dir.path().join(".git")).await;
        assert_eq!(state.paths.as_known().unwrap().untracked, 1);
    }
}
