use git_process::{GitCall, Intent, ProcessLayer};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActiveOperationKind {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveOperationDetail {
    pub kind: ActiveOperationKind,
    pub title: String,
    pub description: String,
    pub conflicting_files: Vec<String>,
    pub head_commit: Option<String>,
    pub target_ref: Option<String>,
    pub current_commit_msg: Option<String>,
    pub rebase_current_step: Option<usize>,
    pub rebase_total_steps: Option<usize>,
    pub can_skip: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirtyTreeDetails {
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MergeOptions {
    pub no_ff: bool,
    pub ff_only: bool,
    pub squash: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum MergeOutcome {
    Success {
        new_head: String,
        is_fast_forward: bool,
        is_squash: bool,
        message: String,
    },
    Conflict {
        conflicting_files: Vec<String>,
        message: String,
    },
    DirtyTreeRefusal(DirtyTreeDetails),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CherryPickOptions {
    pub no_commit: bool,
    pub signoff: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum CherryPickOutcome {
    Success {
        new_head: String,
        message: String,
    },
    Conflict {
        conflicting_files: Vec<String>,
        message: String,
    },
    DirtyTreeRefusal(DirtyTreeDetails),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevertOptions {
    pub no_commit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum RevertOutcome {
    Success {
        new_head: String,
        message: String,
    },
    Conflict {
        conflicting_files: Vec<String>,
        message: String,
    },
    DirtyTreeRefusal(DirtyTreeDetails),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RebaseAction {
    Pick,
    Reword,
    Edit,
    Squash,
    Fixup,
    Drop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RebasePlanItem {
    pub commit: String,
    pub short_commit: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    pub action: RebaseAction,
    pub new_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum RebaseOutcome {
    Success {
        new_head: String,
        message: String,
    },
    Paused {
        stopped_sha: Option<String>,
        conflicting_files: Vec<String>,
        message: String,
    },
    DirtyTreeRefusal(DirtyTreeDetails),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum OperationStepOutcome {
    Completed {
        new_head: String,
        message: String,
    },
    StillInProgress(ActiveOperationDetail),
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AbortOutcome {
    pub operation: String,
    pub restored_head: String,
    pub restored_head_short: String,
    pub restored_head_subject: String,
    pub restored_branch: String,
    pub working_tree_clean: bool,
    pub summary: String,
}

async fn get_git_dir(layer: &ProcessLayer, root: &Path) -> Result<PathBuf, String> {
    let res = layer
        .run(
            GitCall::new(root, ["rev-parse", "--absolute-git-dir"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error resolving git dir: {e}"))?;
    if !res.ok() {
        return Err(format!("Failed to resolve git dir: {}", res.stderr.trim()));
    }
    Ok(PathBuf::from(res.stdout_utf8_lossy().trim()))
}

pub async fn query_conflicting_files(
    layer: &ProcessLayer,
    root: &Path,
) -> Result<Vec<String>, String> {
    let res = layer
        .run(
            GitCall::new(root, ["diff", "--name-only", "--diff-filter=U"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error querying conflicting files: {e}"))?;

    if !res.ok() {
        return Err(format!("git diff failed: {}", res.stderr.trim()));
    }

    let files = res
        .stdout_utf8_lossy()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    Ok(files)
}

pub async fn check_dirty_tree(
    layer: &ProcessLayer,
    root: &Path,
) -> Result<Option<DirtyTreeDetails>, String> {
    let res = layer
        .run(
            GitCall::new(
                root,
                ["status", "--porcelain=v2", "-z", "--untracked-files=all"],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error checking status: {e}"))?;

    if !res.ok() {
        return Err(format!("git status failed: {}", res.stderr.trim()));
    }

    let stdout = &res.stdout;
    let mut staged = 0usize;
    let mut unstaged = 0usize;
    let mut untracked = 0usize;

    let mut i = 0;
    while i < stdout.len() {
        let end = stdout[i..]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(stdout.len() - i);
        let entry = &stdout[i..i + end];
        i += end + 1;

        if entry.is_empty() {
            continue;
        }

        if entry.starts_with(b"1 ") || entry.starts_with(b"2 ") {
            if let Ok(s) = std::str::from_utf8(entry) {
                let parts: Vec<&str> = s.split(' ').collect();
                if parts.len() >= 2 && parts[1].len() >= 2 {
                    let xy = parts[1].as_bytes();
                    if xy[0] != b'.' {
                        staged += 1;
                    }
                    if xy[1] != b'.' {
                        unstaged += 1;
                    }
                }
            }
            if entry.starts_with(b"2 ") && i < stdout.len() {
                let skip = stdout[i..]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(stdout.len() - i);
                i += skip + 1;
            }
        } else if entry.starts_with(b"u ") {
            unstaged += 1;
        } else if entry.starts_with(b"? ") {
            untracked += 1;
        }
    }

    if staged > 0 || unstaged > 0 {
        let summary = format!(
            "Working copy has {} staged and {} unstaged modification(s). Commit, stash or discard before starting an integration operation.",
            staged, unstaged
        );
        Ok(Some(DirtyTreeDetails {
            staged_count: staged,
            unstaged_count: unstaged,
            untracked_count: untracked,
            summary,
        }))
    } else {
        Ok(None)
    }
}

pub async fn query_active_operation(
    layer: &ProcessLayer,
    root: &Path,
) -> Result<Option<ActiveOperationDetail>, String> {
    let git_dir = match get_git_dir(layer, root).await {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    let conflicting_files = query_conflicting_files(layer, root).await.unwrap_or_default();

    if git_dir.join("MERGE_HEAD").exists() {
        let merge_head = std::fs::read_to_string(git_dir.join("MERGE_HEAD"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let merge_msg = std::fs::read_to_string(git_dir.join("MERGE_MSG"))
            .map(|s| s.trim().to_string())
            .ok();

        return Ok(Some(ActiveOperationDetail {
            kind: ActiveOperationKind::Merge,
            title: "Merge in progress".to_string(),
            description: if conflicting_files.is_empty() {
                "Merge commit is ready to be finalized.".to_string()
            } else {
                format!(
                    "Merge conflicts detected in {} file(s). Resolve all conflicts before continuing.",
                    conflicting_files.len()
                )
            },
            conflicting_files,
            head_commit: Some(merge_head),
            target_ref: None,
            current_commit_msg: merge_msg,
            rebase_current_step: None,
            rebase_total_steps: None,
            can_skip: false,
        }));
    }

    let rebase_merge_dir = git_dir.join("rebase-merge");
    let rebase_apply_dir = git_dir.join("rebase-apply");
    if rebase_merge_dir.is_dir() || rebase_apply_dir.is_dir() {
        let active_dir = if rebase_merge_dir.is_dir() {
            rebase_merge_dir
        } else {
            rebase_apply_dir
        };

        let stopped_sha = std::fs::read_to_string(active_dir.join("stopped-sha"))
            .map(|s| s.trim().to_string())
            .ok();
        let head_name = std::fs::read_to_string(active_dir.join("head-name"))
            .map(|s| s.trim().to_string())
            .ok();
        let msg = std::fs::read_to_string(active_dir.join("msg"))
            .or_else(|_| std::fs::read_to_string(active_dir.join("message")))
            .map(|s| s.trim().to_string())
            .ok();

        let done_count = std::fs::read_to_string(active_dir.join("done"))
            .map(|s| s.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count())
            .unwrap_or(0);
        let todo_count = std::fs::read_to_string(active_dir.join("git-rebase-todo"))
            .map(|s| s.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count())
            .unwrap_or(0);
        let total = done_count + todo_count;

        return Ok(Some(ActiveOperationDetail {
            kind: ActiveOperationKind::Rebase,
            title: "Interactive rebase in progress".to_string(),
            description: if conflicting_files.is_empty() {
                format!("Rebase paused at step {} of {}.", done_count, total)
            } else {
                format!(
                    "Rebase paused with {} conflict(s) at step {} of {}.",
                    conflicting_files.len(),
                    done_count,
                    total
                )
            },
            conflicting_files,
            head_commit: stopped_sha,
            target_ref: head_name,
            current_commit_msg: msg,
            rebase_current_step: Some(done_count),
            rebase_total_steps: Some(total),
            can_skip: true,
        }));
    }

    if git_dir.join("CHERRY_PICK_HEAD").exists() {
        let cp_head = std::fs::read_to_string(git_dir.join("CHERRY_PICK_HEAD"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        return Ok(Some(ActiveOperationDetail {
            kind: ActiveOperationKind::CherryPick,
            title: "Cherry-pick in progress".to_string(),
            description: if conflicting_files.is_empty() {
                "Cherry-pick changes applied. Ready to continue or abort.".to_string()
            } else {
                format!(
                    "Cherry-pick conflicts in {} file(s). Resolve all conflicts before continuing.",
                    conflicting_files.len()
                )
            },
            conflicting_files,
            head_commit: Some(cp_head),
            target_ref: None,
            current_commit_msg: None,
            rebase_current_step: None,
            rebase_total_steps: None,
            can_skip: true,
        }));
    }

    if git_dir.join("REVERT_HEAD").exists() {
        let rev_head = std::fs::read_to_string(git_dir.join("REVERT_HEAD"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        return Ok(Some(ActiveOperationDetail {
            kind: ActiveOperationKind::Revert,
            title: "Revert in progress".to_string(),
            description: if conflicting_files.is_empty() {
                "Revert changes applied. Ready to continue or abort.".to_string()
            } else {
                format!(
                    "Revert conflicts in {} file(s). Resolve all conflicts before continuing.",
                    conflicting_files.len()
                )
            },
            conflicting_files,
            head_commit: Some(rev_head),
            target_ref: None,
            current_commit_msg: None,
            rebase_current_step: None,
            rebase_total_steps: None,
            can_skip: true,
        }));
    }

    Ok(None)
}

pub async fn start_merge(
    layer: &ProcessLayer,
    root: &Path,
    target_ref: &str,
    options: MergeOptions,
) -> Result<MergeOutcome, String> {
    if let Some(dirty) = check_dirty_tree(layer, root).await? {
        return Ok(MergeOutcome::DirtyTreeRefusal(dirty));
    }

    let mut args = vec!["merge".to_string()];
    if options.no_ff {
        args.push("--no-ff".to_string());
    }
    if options.ff_only {
        args.push("--ff-only".to_string());
    }
    if options.squash {
        args.push("--squash".to_string());
    }
    if let Some(msg) = &options.message {
        let trimmed = msg.trim();
        if !trimmed.is_empty() {
            args.push("-m".to_string());
            args.push(trimmed.to_string());
        }
    }
    args.push(target_ref.to_string());

    let res = layer
        .run(
            GitCall::new(root, args),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error executing merge: {e}"))?;

    let stdout_str = res.stdout_utf8_lossy();
    let is_ff = stdout_str.contains("Fast-forward");

    if res.ok() {
        let head_res = layer
            .run(
                GitCall::new(root, ["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .map_err(|e| format!("Failed to read HEAD after merge: {e}"))?;

        let new_head = head_res.stdout_utf8_lossy().trim().to_string();
        return Ok(MergeOutcome::Success {
            new_head,
            is_fast_forward: is_ff,
            is_squash: options.squash,
            message: stdout_str.trim().to_string(),
        });
    }

    let conflicts = query_conflicting_files(layer, root).await.unwrap_or_default();
    let git_dir = get_git_dir(layer, root).await.unwrap_or_default();
    if !conflicts.is_empty() || git_dir.join("MERGE_HEAD").exists() {
        return Ok(MergeOutcome::Conflict {
            conflicting_files: conflicts,
            message: format!("{}\n{}", res.stderr.trim(), stdout_str.trim()),
        });
    }

    Err(format!("git merge failed: {}", res.stderr.trim()))
}

pub async fn start_cherry_pick(
    layer: &ProcessLayer,
    root: &Path,
    commit_ref: &str,
    options: CherryPickOptions,
) -> Result<CherryPickOutcome, String> {
    if let Some(dirty) = check_dirty_tree(layer, root).await? {
        return Ok(CherryPickOutcome::DirtyTreeRefusal(dirty));
    }

    let mut args = vec!["cherry-pick".to_string()];
    if options.no_commit {
        args.push("-n".to_string());
    }
    if options.signoff {
        args.push("-s".to_string());
    }
    args.push(commit_ref.to_string());

    let res = layer
        .run(
            GitCall::new(root, args),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error executing cherry-pick: {e}"))?;

    if res.ok() {
        let head_res = layer
            .run(
                GitCall::new(root, ["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .map_err(|e| format!("Failed to read HEAD after cherry-pick: {e}"))?;

        return Ok(CherryPickOutcome::Success {
            new_head: head_res.stdout_utf8_lossy().trim().to_string(),
            message: "Cherry-pick completed cleanly.".to_string(),
        });
    }

    let conflicts = query_conflicting_files(layer, root).await.unwrap_or_default();
    let git_dir = get_git_dir(layer, root).await.unwrap_or_default();
    if !conflicts.is_empty() || git_dir.join("CHERRY_PICK_HEAD").exists() {
        return Ok(CherryPickOutcome::Conflict {
            conflicting_files: conflicts,
            message: format!("{}\n{}", res.stderr.trim(), res.stdout_utf8_lossy().trim()),
        });
    }

    Err(format!("git cherry-pick failed: {}", res.stderr.trim()))
}

pub async fn start_revert(
    layer: &ProcessLayer,
    root: &Path,
    commit_ref: &str,
    options: RevertOptions,
) -> Result<RevertOutcome, String> {
    if let Some(dirty) = check_dirty_tree(layer, root).await? {
        return Ok(RevertOutcome::DirtyTreeRefusal(dirty));
    }

    let mut args = vec!["revert".to_string(), "--no-edit".to_string()];
    if options.no_commit {
        args.push("-n".to_string());
    }
    args.push(commit_ref.to_string());

    let res = layer
        .run(
            GitCall::new(root, args),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error executing revert: {e}"))?;

    if res.ok() {
        let head_res = layer
            .run(
                GitCall::new(root, ["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .map_err(|e| format!("Failed to read HEAD after revert: {e}"))?;

        return Ok(RevertOutcome::Success {
            new_head: head_res.stdout_utf8_lossy().trim().to_string(),
            message: "Revert completed cleanly.".to_string(),
        });
    }

    let conflicts = query_conflicting_files(layer, root).await.unwrap_or_default();
    let git_dir = get_git_dir(layer, root).await.unwrap_or_default();
    if !conflicts.is_empty() || git_dir.join("REVERT_HEAD").exists() {
        return Ok(RevertOutcome::Conflict {
            conflicting_files: conflicts,
            message: format!("{}\n{}", res.stderr.trim(), res.stdout_utf8_lossy().trim()),
        });
    }

    Err(format!("git revert failed: {}", res.stderr.trim()))
}

pub async fn query_rebase_plan(
    layer: &ProcessLayer,
    root: &Path,
    base_ref: &str,
) -> Result<Vec<RebasePlanItem>, String> {
    let fmt = "%H%x00%h%x00%an%x00%cI%x00%s";
    let range = format!("{base_ref}..HEAD");

    let res = layer
        .run(
            GitCall::new(
                root,
                ["log", "--reverse", &format!("--format={fmt}"), &range],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error querying rebase commits: {e}"))?;

    if !res.ok() {
        return Err(format!("git log failed: {}", res.stderr.trim()));
    }

    let stdout = res.stdout_utf8_lossy();
    let mut items = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parts: Vec<&str> = trimmed.split('\x00').collect();
        if parts.len() >= 5 {
            items.push(RebasePlanItem {
                commit: parts[0].to_string(),
                short_commit: parts[1].to_string(),
                author: parts[2].to_string(),
                date: parts[3].to_string(),
                subject: parts[4].to_string(),
                action: RebaseAction::Pick,
                new_message: None,
            });
        }
    }

    Ok(items)
}

pub async fn start_interactive_rebase(
    layer: &ProcessLayer,
    root: &Path,
    base_ref: &str,
    plan: Vec<RebasePlanItem>,
) -> Result<RebaseOutcome, String> {
    if let Some(dirty) = check_dirty_tree(layer, root).await? {
        return Ok(RebaseOutcome::DirtyTreeRefusal(dirty));
    }

    if plan.is_empty() {
        return Err("Rebase plan cannot be empty.".to_string());
    }

    let mut todo_lines = String::new();
    for item in &plan {
        match item.action {
            RebaseAction::Pick => {
                todo_lines.push_str(&format!("pick {} {}\n", item.commit, item.subject));
            }
            RebaseAction::Reword => {
                if let Some(msg) = &item.new_message {
                    let trimmed = msg.trim();
                    if !trimmed.is_empty() {
                        todo_lines.push_str(&format!("pick {} {}\n", item.commit, item.subject));
                        let escaped = trimmed.replace('\\', "\\\\").replace('"', "\\\"");
                        todo_lines.push_str(&format!("exec git commit --amend -m \"{}\"\n", escaped));
                        continue;
                    }
                }
                todo_lines.push_str(&format!("reword {} {}\n", item.commit, item.subject));
            }
            RebaseAction::Edit => {
                todo_lines.push_str(&format!("edit {} {}\n", item.commit, item.subject));
            }
            RebaseAction::Squash => {
                if let Some(msg) = &item.new_message {
                    let trimmed = msg.trim();
                    if !trimmed.is_empty() {
                        todo_lines.push_str(&format!("fixup {} {}\n", item.commit, item.subject));
                        let escaped = trimmed.replace('\\', "\\\\").replace('"', "\\\"");
                        todo_lines.push_str(&format!("exec git commit --amend -m \"{}\"\n", escaped));
                        continue;
                    }
                }
                todo_lines.push_str(&format!("squash {} {}\n", item.commit, item.subject));
            }
            RebaseAction::Fixup => {
                todo_lines.push_str(&format!("fixup {} {}\n", item.commit, item.subject));
            }
            RebaseAction::Drop => {
                todo_lines.push_str(&format!("drop {} {}\n", item.commit, item.subject));
            }
        }
    }

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp_plan = std::env::temp_dir().join(format!("gittree_rebase_todo_{ts}.txt"));
    std::fs::write(&temp_plan, &todo_lines)
        .map_err(|e| format!("Failed to write rebase plan: {e}"))?;

    let call = GitCall::new(root, ["rebase", "-i", base_ref])
        .with_env("GIT_SEQUENCE_EDITOR", format!("cp \"{}\"", temp_plan.display()))
        .with_env("GIT_EDITOR", "true");

    let res = layer
        .run(call, Intent::Write, CancellationToken::new())
        .await;

    let _ = std::fs::remove_file(&temp_plan);

    let res = res.map_err(|e| format!("Process error executing rebase: {e}"))?;

    if res.ok() {
        let head_res = layer
            .run(
                GitCall::new(root, ["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .map_err(|e| format!("Failed to read HEAD after rebase: {e}"))?;

        return Ok(RebaseOutcome::Success {
            new_head: head_res.stdout_utf8_lossy().trim().to_string(),
            message: "Interactive rebase executed successfully.".to_string(),
        });
    }

    let git_dir = get_git_dir(layer, root).await.unwrap_or_default();
    let rebase_active = git_dir.join("rebase-merge").is_dir() || git_dir.join("rebase-apply").is_dir();

    if rebase_active {
        let active_dir = if git_dir.join("rebase-merge").is_dir() {
            git_dir.join("rebase-merge")
        } else {
            git_dir.join("rebase-apply")
        };
        let stopped_sha = std::fs::read_to_string(active_dir.join("stopped-sha"))
            .map(|s| s.trim().to_string())
            .ok();
        let conflicts = query_conflicting_files(layer, root).await.unwrap_or_default();

        return Ok(RebaseOutcome::Paused {
            stopped_sha,
            conflicting_files: conflicts,
            message: format!("{}\n{}", res.stderr.trim(), res.stdout_utf8_lossy().trim()),
        });
    }

    Err(format!("git rebase failed: {}", res.stderr.trim()))
}

pub async fn continue_operation(
    layer: &ProcessLayer,
    root: &Path,
) -> Result<OperationStepOutcome, String> {
    let active = query_active_operation(layer, root).await?;
    let active = match active {
        Some(a) => a,
        None => return Err("No integration operation is currently in progress.".to_string()),
    };

    let call = match active.kind {
        ActiveOperationKind::Merge => GitCall::new(root, ["commit", "--no-edit"]),
        ActiveOperationKind::Rebase => {
            GitCall::new(root, ["rebase", "--continue"]).with_env("GIT_EDITOR", "true")
        }
        ActiveOperationKind::CherryPick => {
            GitCall::new(root, ["cherry-pick", "--continue"]).with_env("GIT_EDITOR", "true")
        }
        ActiveOperationKind::Revert => {
            GitCall::new(root, ["revert", "--continue"]).with_env("GIT_EDITOR", "true")
        }
    };

    let res = layer
        .run(call, Intent::Write, CancellationToken::new())
        .await
        .map_err(|e| format!("Process error continuing operation: {e}"))?;

    let post_active = query_active_operation(layer, root).await?;
    if let Some(detail) = post_active {
        return Ok(OperationStepOutcome::StillInProgress(detail));
    }

    if res.ok() {
        let head_res = layer
            .run(
                GitCall::new(root, ["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .map_err(|e| format!("Failed to read HEAD after continue: {e}"))?;

        return Ok(OperationStepOutcome::Completed {
            new_head: head_res.stdout_utf8_lossy().trim().to_string(),
            message: "Operation completed successfully.".to_string(),
        });
    }

    Ok(OperationStepOutcome::Failed {
        message: format!("{}\n{}", res.stderr.trim(), res.stdout_utf8_lossy().trim()),
    })
}

pub async fn skip_operation(
    layer: &ProcessLayer,
    root: &Path,
) -> Result<OperationStepOutcome, String> {
    let active = query_active_operation(layer, root).await?;
    let active = match active {
        Some(a) => a,
        None => return Err("No integration operation is currently in progress.".to_string()),
    };

    let call = match active.kind {
        ActiveOperationKind::Merge => {
            return Err("Merge operations do not support skipping. You may continue or abort.".to_string());
        }
        ActiveOperationKind::Rebase => {
            GitCall::new(root, ["rebase", "--skip"]).with_env("GIT_EDITOR", "true")
        }
        ActiveOperationKind::CherryPick => {
            GitCall::new(root, ["cherry-pick", "--skip"]).with_env("GIT_EDITOR", "true")
        }
        ActiveOperationKind::Revert => {
            GitCall::new(root, ["revert", "--skip"]).with_env("GIT_EDITOR", "true")
        }
    };

    let res = layer
        .run(call, Intent::Write, CancellationToken::new())
        .await
        .map_err(|e| format!("Process error skipping operation: {e}"))?;

    let post_active = query_active_operation(layer, root).await?;
    if let Some(detail) = post_active {
        return Ok(OperationStepOutcome::StillInProgress(detail));
    }

    if res.ok() {
        let head_res = layer
            .run(
                GitCall::new(root, ["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .map_err(|e| format!("Failed to read HEAD after skip: {e}"))?;

        return Ok(OperationStepOutcome::Completed {
            new_head: head_res.stdout_utf8_lossy().trim().to_string(),
            message: "Skipped step; operation completed successfully.".to_string(),
        });
    }

    Ok(OperationStepOutcome::Failed {
        message: format!("{}\n{}", res.stderr.trim(), res.stdout_utf8_lossy().trim()),
    })
}

pub async fn abort_operation(layer: &ProcessLayer, root: &Path) -> Result<AbortOutcome, String> {
    let active = query_active_operation(layer, root).await?;
    let active = match active {
        Some(a) => a,
        None => return Err("No integration operation is currently in progress to abort.".to_string()),
    };

    let (op_name, call) = match active.kind {
        ActiveOperationKind::Merge => ("Merge", GitCall::new(root, ["merge", "--abort"])),
        ActiveOperationKind::Rebase => ("Rebase", GitCall::new(root, ["rebase", "--abort"])),
        ActiveOperationKind::CherryPick => ("Cherry-pick", GitCall::new(root, ["cherry-pick", "--abort"])),
        ActiveOperationKind::Revert => ("Revert", GitCall::new(root, ["revert", "--abort"])),
    };

    let res = layer
        .run(call, Intent::Write, CancellationToken::new())
        .await
        .map_err(|e| format!("Process error aborting operation: {e}"))?;

    if !res.ok() {
        return Err(format!("git {op_name} --abort failed: {}", res.stderr.trim()));
    }

    let head_res = layer
        .run(
            GitCall::new(root, ["rev-parse", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Failed to read restored HEAD: {e}"))?;
    let restored_head = head_res.stdout_utf8_lossy().trim().to_string();

    let short_res = layer
        .run(
            GitCall::new(root, ["rev-parse", "--short", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Failed to read short HEAD: {e}"))?;
    let restored_head_short = short_res.stdout_utf8_lossy().trim().to_string();

    let subject_res = layer
        .run(
            GitCall::new(root, ["log", "-1", "--format=%s", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Failed to read commit subject: {e}"))?;
    let restored_head_subject = subject_res.stdout_utf8_lossy().trim().to_string();

    let branch_res = layer
        .run(
            GitCall::new(root, ["symbolic-ref", "--short", "-q", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Failed to read branch: {e}"))?;
    let restored_branch = if branch_res.ok() {
        branch_res.stdout_utf8_lossy().trim().to_string()
    } else {
        "HEAD (detached)".to_string()
    };

    let dirty_check = check_dirty_tree(layer, root).await?;
    let clean = dirty_check.is_none();

    let summary = format!(
        "Aborted {op_name} successfully. Restored branch '{restored_branch}' to commit {restored_head_short} ('{restored_head_subject}'). Working copy is {}.",
        if clean { "clean" } else { "modified" }
    );

    Ok(AbortOutcome {
        operation: op_name.to_string(),
        restored_head,
        restored_head_short,
        restored_head_subject,
        restored_branch,
        working_tree_clean: clean,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;
    use tempfile::tempdir;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn create_repo_with_commits() -> (tempfile::TempDir, ProcessLayer) {
        let dir = tempdir().expect("create temp dir");
        let layer = ProcessLayer::new(4, Duration::from_secs(10));
        let path = dir.path();

        git(path, &["init"]);
        git(path, &["config", "user.name", "Test User"]);
        git(path, &["config", "user.email", "test@example.com"]);

        std::fs::write(path.join("file1.txt"), "hello world\n").unwrap();
        git(path, &["add", "file1.txt"]);
        git(path, &["commit", "-m", "initial commit"]);

        (dir, layer)
    }

    #[tokio::test]
    async fn test_merge_dirty_tree_refusal_and_clean_merge() {
        let (dir, layer) = create_repo_with_commits();
        let root = dir.path();

        // Create feature branch with a commit
        git(root, &["checkout", "-b", "feature"]);
        std::fs::write(root.join("feature.txt"), "feature data\n").unwrap();
        git(root, &["add", "feature.txt"]);
        git(root, &["commit", "-m", "feature commit"]);

        // Switch back to master
        git(root, &["checkout", "master"]);

        // Make working tree dirty
        std::fs::write(root.join("dirty.txt"), "dirty untracked data\n").unwrap();
        git(root, &["add", "dirty.txt"]);

        // Start merge while dirty -> should refuse
        let outcome = start_merge(
            &layer,
            root,
            "feature",
            MergeOptions {
                no_ff: true,
                ff_only: false,
                squash: false,
                message: Some("Merge feature".into()),
            },
        )
        .await
        .unwrap();

        match outcome {
            MergeOutcome::DirtyTreeRefusal(details) => {
                assert_eq!(details.staged_count, 1);
            }
            _ => panic!("Expected DirtyTreeRefusal, got {:?}", outcome),
        }

        // Clean up dirty state
        git(root, &["reset", "--hard", "HEAD"]);

        // Now start merge clean
        let clean_outcome = start_merge(
            &layer,
            root,
            "feature",
            MergeOptions {
                no_ff: true,
                ff_only: false,
                squash: false,
                message: Some("Merge feature".into()),
            },
        )
        .await
        .unwrap();

        match clean_outcome {
            MergeOutcome::Success { new_head, .. } => {
                assert!(!new_head.is_empty());
            }
            _ => panic!("Expected Success, got {:?}", clean_outcome),
        }

        assert!(query_active_operation(&layer, root).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_interactive_rebase_plan_execution_and_abort() {
        let (dir, layer) = create_repo_with_commits();
        let root = dir.path();

        let base_head = git(root, &["rev-parse", "HEAD"]).trim().to_string();

        // Add 2 commits
        for i in 2..=3 {
            let filename = format!("file{i}.txt");
            let msg = format!("commit {i}");
            std::fs::write(root.join(&filename), format!("content {i}\n")).unwrap();
            git(root, &["add", &filename]);
            git(root, &["commit", "-m", &msg]);
        }

        // Query rebase plan
        let plan = query_rebase_plan(&layer, root, &base_head).await.unwrap();
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].subject, "commit 2");
        assert_eq!(plan[1].subject, "commit 3");

        // Prepare modified plan: reword commit 2, drop commit 3
        let mut modified_plan = plan.clone();
        modified_plan[0].action = RebaseAction::Reword;
        modified_plan[0].new_message = Some("reworded commit 2".to_string());
        modified_plan[1].action = RebaseAction::Drop;

        let rebase_res = start_interactive_rebase(&layer, root, &base_head, modified_plan)
            .await
            .unwrap();

        match rebase_res {
            RebaseOutcome::Success { new_head, .. } => {
                assert!(!new_head.is_empty());
            }
            _ => panic!("Expected Success, got {:?}", rebase_res),
        }

        // Verify log after rebase: commit 3 is dropped, commit 2 has reworded message
        let log = git(root, &["log", "--format=%s"]);
        assert!(log.contains("reworded commit 2"));
        assert!(!log.contains("commit 3"));
    }

    #[tokio::test]
    async fn test_merge_conflict_and_abort_confirmation() {
        let (dir, layer) = create_repo_with_commits();
        let root = dir.path();

        // Create branch sideA
        git(root, &["checkout", "-b", "sideA"]);
        std::fs::write(root.join("file1.txt"), "conflict A\n").unwrap();
        git(root, &["commit", "-am", "side A commit"]);

        // Create branch sideB from master
        git(root, &["checkout", "master"]);
        git(root, &["checkout", "-b", "sideB"]);
        std::fs::write(root.join("file1.txt"), "conflict B\n").unwrap();
        git(root, &["commit", "-am", "side B commit"]);

        // Merge sideA into sideB -> conflict!
        let outcome = start_merge(
            &layer,
            root,
            "sideA",
            MergeOptions {
                no_ff: true,
                ff_only: false,
                squash: false,
                message: None,
            },
        )
        .await
        .unwrap();

        match outcome {
            MergeOutcome::Conflict { conflicting_files, .. } => {
                assert!(conflicting_files.contains(&"file1.txt".to_string()));
            }
            _ => panic!("Expected Conflict, got {:?}", outcome),
        }

        // Active operation is detected
        let active = query_active_operation(&layer, root).await.unwrap().expect("active operation");
        assert_eq!(active.kind, ActiveOperationKind::Merge);
        assert_eq!(active.conflicting_files, vec!["file1.txt"]);

        // Abort operation
        let abort = abort_operation(&layer, root).await.unwrap();
        assert_eq!(abort.operation, "Merge");
        assert_eq!(abort.restored_branch, "sideB");
        assert!(abort.working_tree_clean);
        assert!(abort.summary.contains("Restored branch 'sideB'"));

        // Verify active operation is now gone and tree is clean
        assert!(query_active_operation(&layer, root).await.unwrap().is_none());
    }
}
