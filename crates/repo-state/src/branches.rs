use std::path::Path;

use futures::future::join_all;
use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

use crate::history::CommitSummary;
use crate::upstream::{resolve_upstream_basis, UpstreamBasis};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BranchEntry {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
    pub target_commit: String,
    pub commit_subject: String,
    pub upstream: Option<String>,
    pub ahead_behind: Option<(u32, u32)>,
    pub upstream_basis: UpstreamBasis,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BranchComparisonFile {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BranchComparison {
    pub base: String,
    pub target: String,
    pub ahead: u32,
    pub behind: u32,
    pub ahead_commits: Vec<CommitSummary>,
    pub behind_commits: Vec<CommitSummary>,
    pub changed_files: Vec<BranchComparisonFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status")]
pub enum CheckoutOutcome {
    Success,
    Conflict {
        target: String,
        conflicting_files: Vec<String>,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status")]
pub enum DeleteBranchOutcome {
    Deleted,
    UnmergedGuard {
        branch: String,
        tip_commit: String,
        commits: Vec<CommitSummary>,
        recovery_hint: String,
    },
}

const FIELD_SEP: char = '\u{1f}';

fn parse_commit_record(record: &str) -> Option<CommitSummary> {
    if record.is_empty() {
        return None;
    }
    let mut fields = record.split(FIELD_SEP);
    let sha = fields.next()?.to_string();
    let parents = fields
        .next()?
        .split(' ')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect();
    let author_name = fields.next()?.to_string();
    let author_email = fields.next()?.to_string();
    let author_date = fields.next()?.to_string();
    let subject = fields.next().unwrap_or("").to_string();
    Some(CommitSummary {
        sha,
        parents,
        author_name,
        author_email,
        author_date,
        subject,
        rename_from: None,
        path_at_commit: None,
    })
}

fn parse_track_counts(track: &str) -> (u32, u32) {
    let mut ahead = 0;
    let mut behind = 0;
    for part in track.split(',') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("ahead ") {
            ahead = rest.parse::<u32>().unwrap_or(0);
        } else if let Some(rest) = part.strip_prefix("behind ") {
            behind = rest.parse::<u32>().unwrap_or(0);
        }
    }
    (ahead, behind)
}

pub async fn query_branches(layer: &ProcessLayer, root: &Path) -> Result<Vec<BranchEntry>, String> {
    let format = "%(refname:short)%00%(HEAD)%00%(objectname:short)%00%(contents:subject)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(refname)%00";
    let call = GitCall::new(
        root,
        [
            "for-each-ref",
            &format!("--format={format}"),
            "refs/heads",
            "refs/remotes",
        ],
    );

    let result = layer
        .run(call, Intent::Read, CancellationToken::new())
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git for-each-ref failed: {}", result.stderr.trim()));
    }

    let stdout = result.stdout_utf8_lossy();
    let mut raw_entries = Vec::new();

    for block in stdout.split('\n') {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let parts: Vec<&str> = block.split('\0').collect();
        if parts.len() < 7 {
            continue;
        }
        let short_name = parts[0].to_string();
        let head_marker = parts[1];
        let target_commit = parts[2].to_string();
        let commit_subject = parts[3].to_string();
        let upstream_short = parts[4].to_string();
        let upstream_track = parts[5].to_string();
        let full_ref = parts[6].to_string();

        if short_name.ends_with("/HEAD") || full_ref.ends_with("/HEAD") {
            continue;
        }

        let is_remote = full_ref.starts_with("refs/remotes/");
        let is_head = head_marker.trim() == "*";

        raw_entries.push((
            short_name,
            is_head,
            is_remote,
            target_commit,
            commit_subject,
            upstream_short,
            upstream_track,
        ));
    }

    // Resolve upstream basis and ahead/behind for each entry
    let futures = raw_entries.into_iter().map(
        |(
            name,
            is_head,
            is_remote,
            target_commit,
            commit_subject,
            upstream_short,
            upstream_track,
        )| {
            async move {
                if is_remote {
                    BranchEntry {
                        name,
                        is_head,
                        is_remote: true,
                        target_commit,
                        commit_subject,
                        upstream: None,
                        ahead_behind: None,
                        upstream_basis: UpstreamBasis::None,
                    }
                } else if !upstream_short.is_empty() {
                    let ahead_behind = parse_track_counts(&upstream_track);
                    BranchEntry {
                        name,
                        is_head,
                        is_remote: false,
                        target_commit,
                        commit_subject,
                        upstream: Some(upstream_short.clone()),
                        ahead_behind: Some(ahead_behind),
                        upstream_basis: UpstreamBasis::Configured {
                            refname: upstream_short,
                        },
                    }
                } else {
                    let basis = resolve_upstream_basis(layer, root, Some(&name)).await;
                    let (upstream, ahead_behind) = match &basis {
                        UpstreamBasis::Inferred { refname } => {
                            let range = format!("{refname}...{name}");
                            let count_call =
                                GitCall::new(root, ["rev-list", "--left-right", "--count", &range]);
                            let ab = match layer
                                .run(count_call, Intent::Read, CancellationToken::new())
                                .await
                            {
                                Ok(res) if res.ok() => {
                                    let stdout = res.stdout_utf8_lossy();
                                    let mut nums = stdout
                                        .split_whitespace()
                                        .filter_map(|s| s.parse::<u32>().ok());
                                    let behind = nums.next().unwrap_or(0);
                                    let ahead = nums.next().unwrap_or(0);
                                    Some((ahead, behind))
                                }
                                _ => None,
                            };
                            (Some(refname.clone()), ab)
                        }
                        _ => (None, None),
                    };

                    BranchEntry {
                        name,
                        is_head,
                        is_remote: false,
                        target_commit,
                        commit_subject,
                        upstream,
                        ahead_behind,
                        upstream_basis: basis,
                    }
                }
            }
        },
    );

    Ok(join_all(futures).await)
}

pub async fn create_branch(
    layer: &ProcessLayer,
    root: &Path,
    name: &str,
    start_point: Option<&str>,
    checkout: bool,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('-') {
        return Err("Invalid branch name".to_string());
    }

    let mut args = if checkout {
        vec!["checkout".to_string(), "-b".to_string(), name.to_string()]
    } else {
        vec!["branch".to_string(), name.to_string()]
    };

    if let Some(sp) = start_point {
        let sp = sp.trim();
        if !sp.is_empty() {
            args.push(sp.to_string());
        }
    }

    let result = layer
        .run(
            GitCall::new(root, args),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;

    if !result.ok() {
        return Err(format!("Failed to create branch: {}", result.stderr.trim()));
    }

    Ok(())
}

pub async fn create_tracking_branch(
    layer: &ProcessLayer,
    root: &Path,
    name: &str,
    remote_branch: &str,
    checkout: bool,
) -> Result<(), String> {
    let name = name.trim();
    let remote_branch = remote_branch.trim();
    if name.is_empty() || name.starts_with('-') {
        return Err("Invalid branch name".to_string());
    }
    if remote_branch.is_empty() {
        return Err("Remote branch cannot be empty".to_string());
    }

    let args = if checkout {
        vec![
            "checkout".to_string(),
            "-b".to_string(),
            name.to_string(),
            "--track".to_string(),
            remote_branch.to_string(),
        ]
    } else {
        vec![
            "branch".to_string(),
            "--track".to_string(),
            name.to_string(),
            remote_branch.to_string(),
        ]
    };

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
            "Failed to create tracking branch: {}",
            result.stderr.trim()
        ));
    }

    Ok(())
}

pub async fn rename_branch(
    layer: &ProcessLayer,
    root: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<(), String> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    if new_name.is_empty() || new_name.starts_with('-') {
        return Err("Invalid new branch name".to_string());
    }

    let result = layer
        .run(
            GitCall::new(root, ["branch", "-m", old_name, new_name]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;

    if !result.ok() {
        return Err(format!("Failed to rename branch: {}", result.stderr.trim()));
    }

    Ok(())
}

fn parse_checkout_conflict_files(stderr: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut in_list = false;

    for line in stderr.lines() {
        if line.contains("would be overwritten by checkout:") {
            in_list = true;
            continue;
        }
        if in_list {
            if line.starts_with("Please") || line.starts_with("Aborting") {
                break;
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                files.push(trimmed.to_string());
            }
        }
    }

    files
}

pub async fn checkout_branch(
    layer: &ProcessLayer,
    root: &Path,
    name: &str,
) -> Result<CheckoutOutcome, String> {
    let name = name.trim();
    let result = layer
        .run(
            GitCall::new(root, ["checkout", name]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;

    if result.ok() {
        return Ok(CheckoutOutcome::Success);
    }

    let stderr = result.stderr;
    if stderr.contains("would be overwritten by checkout:") {
        let conflicting_files = parse_checkout_conflict_files(&stderr);
        return Ok(CheckoutOutcome::Conflict {
            target: name.to_string(),
            conflicting_files,
            message: stderr.trim().to_string(),
        });
    }

    Err(format!("Checkout failed: {}", stderr.trim()))
}

pub async fn stash_and_checkout(
    layer: &ProcessLayer,
    root: &Path,
    target: &str,
    message: Option<&str>,
) -> Result<(), String> {
    let stash_msg = message
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Auto-stash before checkout of {target}"));

    // Push stash including untracked files
    let stash_result = layer
        .run(
            GitCall::new(root, ["stash", "push", "-u", "-m", &stash_msg]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;

    if !stash_result.ok() {
        return Err(format!("Stash failed: {}", stash_result.stderr.trim()));
    }

    // Now checkout target branch
    let checkout_result = layer
        .run(
            GitCall::new(root, ["checkout", target]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;

    if !checkout_result.ok() {
        return Err(format!(
            "Checkout after stash failed: {}",
            checkout_result.stderr.trim()
        ));
    }

    Ok(())
}

pub async fn delete_branch(
    layer: &ProcessLayer,
    root: &Path,
    name: &str,
    force: bool,
) -> Result<DeleteBranchOutcome, String> {
    let name = name.trim();
    let flag = if force { "-D" } else { "-d" };

    let result = layer
        .run(
            GitCall::new(root, ["branch", flag, name]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;

    if result.ok() {
        return Ok(DeleteBranchOutcome::Deleted);
    }

    let stderr = result.stderr;
    if !force && stderr.contains("not fully merged") {
        // Query unmerged commits on the branch not in HEAD
        let fmt = format!("--format=%H{0}%P{0}%an{0}%ae{0}%ad{0}%s", FIELD_SEP);
        let log_call = GitCall::new(root, ["log", &fmt, name, "--not", "HEAD"]);
        let commits = match layer
            .run(log_call, Intent::Read, CancellationToken::new())
            .await
        {
            Ok(res) if res.ok() => res
                .stdout_utf8_lossy()
                .lines()
                .filter_map(parse_commit_record)
                .collect(),
            _ => Vec::new(),
        };

        // Query tip commit
        let rev_call = GitCall::new(root, ["rev-parse", "--short", name]);
        let tip_commit = match layer
            .run(rev_call, Intent::Read, CancellationToken::new())
            .await
        {
            Ok(res) if res.ok() => res.stdout_utf8_lossy().trim().to_string(),
            _ => name.to_string(),
        };

        let recovery_hint = format!(
            "Deleting branch '{name}' will make these commit(s) unreachable from HEAD. To recover them later, check git reflog or recreate the branch: git branch {name} {tip_commit}"
        );

        return Ok(DeleteBranchOutcome::UnmergedGuard {
            branch: name.to_string(),
            tip_commit,
            commits,
            recovery_hint,
        });
    }

    Err(format!("Failed to delete branch: {}", stderr.trim()))
}

pub async fn compare_branches(
    layer: &ProcessLayer,
    root: &Path,
    base: &str,
    target: &str,
) -> Result<BranchComparison, String> {
    let base = base.trim();
    let target = target.trim();
    let range = format!("{base}...{target}");

    // 1. rev-list left-right count
    let count_call = GitCall::new(root, ["rev-list", "--left-right", "--count", &range]);
    let count_res = layer
        .run(count_call, Intent::Read, CancellationToken::new())
        .await
        .map_err(|e| e.to_string())?;
    if !count_res.ok() {
        return Err(format!("rev-list failed: {}", count_res.stderr.trim()));
    }

    let stdout = count_res.stdout_utf8_lossy();
    let mut nums = stdout
        .split_whitespace()
        .filter_map(|s| s.parse::<u32>().ok());
    let behind = nums.next().unwrap_or(0);
    let ahead = nums.next().unwrap_or(0);

    let fmt = format!("--format=%H{0}%P{0}%an{0}%ae{0}%ad{0}%s", FIELD_SEP);

    // 2. Ahead commits (in target, not in base)
    let ahead_commits = if ahead > 0 {
        let log_call = GitCall::new(
            root,
            ["log", &fmt, "-n", "100", &format!("{base}..{target}")],
        );
        match layer
            .run(log_call, Intent::Read, CancellationToken::new())
            .await
        {
            Ok(res) if res.ok() => res
                .stdout_utf8_lossy()
                .lines()
                .filter_map(parse_commit_record)
                .collect(),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    // 3. Behind commits (in base, not in target)
    let behind_commits = if behind > 0 {
        let log_call = GitCall::new(
            root,
            ["log", &fmt, "-n", "100", &format!("{target}..{base}")],
        );
        match layer
            .run(log_call, Intent::Read, CancellationToken::new())
            .await
        {
            Ok(res) if res.ok() => res
                .stdout_utf8_lossy()
                .lines()
                .filter_map(parse_commit_record)
                .collect(),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    // 4. Changed files between base and target
    let diff_call = GitCall::new(root, ["diff", "--name-status", &range]);
    let diff_res = layer
        .run(diff_call, Intent::Read, CancellationToken::new())
        .await
        .map_err(|e| e.to_string())?;

    let mut changed_files = Vec::new();
    if diff_res.ok() {
        for line in diff_res.stdout_utf8_lossy().lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split('\t');
            if let (Some(status), Some(path)) = (parts.next(), parts.next()) {
                changed_files.push(BranchComparisonFile {
                    status: status.to_string(),
                    path: path.to_string(),
                });
            }
        }
    }

    Ok(BranchComparison {
        base: base.to_string(),
        target: target.to_string(),
        ahead,
        behind,
        ahead_commits,
        behind_commits,
        changed_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;
    use tempfile::TempDir;

    fn init_repo() -> TempDir {
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
        run(&["config", "user.name", "Test User"]);
        std::fs::write(dir.path().join("init.txt"), "hello world\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "initial commit"]);
        dir
    }

    #[tokio::test]
    async fn test_query_branches_and_inferred_upstream() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Create simulated remote branch refs/remotes/origin/main
        let head_out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let head_sha = String::from_utf8_lossy(&head_out.stdout).trim().to_string();
        std::fs::create_dir_all(dir.path().join(".git/refs/remotes/origin")).unwrap();
        std::fs::write(
            dir.path().join(".git/refs/remotes/origin/main"),
            format!("{head_sha}\n"),
        )
        .unwrap();

        let branches = query_branches(&layer, dir.path()).await.unwrap();
        assert_eq!(branches.len(), 2);

        let local = branches.iter().find(|b| !b.is_remote).unwrap();
        assert_eq!(local.name, "main");
        assert!(local.is_head);
        assert_eq!(
            local.upstream_basis,
            UpstreamBasis::Inferred {
                refname: "origin/main".to_string()
            }
        );
        assert_eq!(local.ahead_behind, Some((0, 0)));

        let remote = branches.iter().find(|b| b.is_remote).unwrap();
        assert_eq!(remote.name, "origin/main");
        assert!(remote.is_remote);
        assert_eq!(remote.upstream_basis, UpstreamBasis::None);
    }

    #[tokio::test]
    async fn test_create_and_rename_branch() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        create_branch(&layer, dir.path(), "feature-1", None, false)
            .await
            .unwrap();

        let branches = query_branches(&layer, dir.path()).await.unwrap();
        assert!(branches.iter().any(|b| b.name == "feature-1"));

        rename_branch(&layer, dir.path(), "feature-1", "feature-renamed")
            .await
            .unwrap();

        let branches = query_branches(&layer, dir.path()).await.unwrap();
        assert!(!branches.iter().any(|b| b.name == "feature-1"));
        assert!(branches.iter().any(|b| b.name == "feature-renamed"));
    }

    #[tokio::test]
    async fn test_create_tracking_branch() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Create remote ref origin/dev
        let head_out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let head_sha = String::from_utf8_lossy(&head_out.stdout).trim().to_string();
        Command::new("git")
            .args(["remote", "add", "origin", "https://example.com/repo.git"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        std::fs::create_dir_all(dir.path().join(".git/refs/remotes/origin")).unwrap();
        std::fs::write(
            dir.path().join(".git/refs/remotes/origin/dev"),
            format!("{head_sha}\n"),
        )
        .unwrap();

        create_tracking_branch(&layer, dir.path(), "dev", "origin/dev", false)
            .await
            .unwrap();

        let branches = query_branches(&layer, dir.path()).await.unwrap();
        let dev = branches.iter().find(|b| b.name == "dev").unwrap();
        assert_eq!(
            dev.upstream_basis,
            UpstreamBasis::Configured {
                refname: "origin/dev".to_string()
            }
        );
        assert_eq!(dev.upstream, Some("origin/dev".to_string()));
    }

    #[tokio::test]
    async fn test_unmerged_branch_deletion_guard() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Create branch feature and commit on it
        create_branch(&layer, dir.path(), "feature", None, true)
            .await
            .unwrap();
        std::fs::write(dir.path().join("feat.txt"), "feature content\n").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "feature commit 1"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        // Switch back to main
        checkout_branch(&layer, dir.path(), "main").await.unwrap();

        // Attempt delete without force
        let outcome = delete_branch(&layer, dir.path(), "feature", false)
            .await
            .unwrap();
        match outcome {
            DeleteBranchOutcome::UnmergedGuard {
                branch,
                tip_commit,
                commits,
                recovery_hint,
            } => {
                assert_eq!(branch, "feature");
                assert!(!tip_commit.is_empty());
                assert_eq!(commits.len(), 1);
                assert_eq!(commits[0].subject, "feature commit 1");
                assert!(recovery_hint.contains("git reflog"));
                assert!(recovery_hint.contains("git branch feature"));
            }
            _ => panic!("Expected unmerged guard"),
        }

        // Force delete works
        let force_outcome = delete_branch(&layer, dir.path(), "feature", true)
            .await
            .unwrap();
        assert_eq!(force_outcome, DeleteBranchOutcome::Deleted);
    }

    #[tokio::test]
    async fn test_checkout_guard_and_stash_checkout() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Create feature branch with different file content
        create_branch(&layer, dir.path(), "feature", None, true)
            .await
            .unwrap();
        std::fs::write(dir.path().join("init.txt"), "modified on feature\n").unwrap();
        Command::new("git")
            .args(["commit", "-am", "change on feature"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        // Switch back to main
        checkout_branch(&layer, dir.path(), "main").await.unwrap();

        // Make uncommitted conflicting change on main
        std::fs::write(dir.path().join("init.txt"), "dirty on main\n").unwrap();

        // Attempt checkout
        let outcome = checkout_branch(&layer, dir.path(), "feature")
            .await
            .unwrap();
        match outcome {
            CheckoutOutcome::Conflict {
                target,
                conflicting_files,
                message,
            } => {
                assert_eq!(target, "feature");
                assert!(conflicting_files.contains(&"init.txt".to_string()));
                assert!(message.contains("overwritten by checkout"));
            }
            _ => panic!("Expected conflict outcome"),
        }

        // Use stash_and_checkout
        stash_and_checkout(&layer, dir.path(), "feature", None)
            .await
            .unwrap();

        // Now HEAD should be on feature
        let branches = query_branches(&layer, dir.path()).await.unwrap();
        let feat = branches.iter().find(|b| b.name == "feature").unwrap();
        assert!(feat.is_head);
    }

    #[tokio::test]
    async fn test_compare_branches() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        create_branch(&layer, dir.path(), "feature", None, true)
            .await
            .unwrap();
        std::fs::write(dir.path().join("f1.txt"), "feature 1\n").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "f1 commit"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        checkout_branch(&layer, dir.path(), "main").await.unwrap();
        std::fs::write(dir.path().join("m1.txt"), "main 1\n").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "m1 commit"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let cmp = compare_branches(&layer, dir.path(), "main", "feature")
            .await
            .unwrap();
        assert_eq!(cmp.ahead, 1);
        assert_eq!(cmp.behind, 1);
        assert_eq!(cmp.ahead_commits.len(), 1);
        assert_eq!(cmp.ahead_commits[0].subject, "f1 commit");
        assert_eq!(cmp.behind_commits.len(), 1);
        assert_eq!(cmp.behind_commits[0].subject, "m1 commit");
        assert!(cmp.changed_files.iter().any(|f| f.path == "f1.txt"));
    }
}
