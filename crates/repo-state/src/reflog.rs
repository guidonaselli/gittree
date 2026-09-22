use git_process::{GitCall, Intent, ProcessLayer};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReflogEntry {
    pub selector: String,
    pub index: usize,
    pub short_sha: String,
    pub commit_sha: String,
    pub operation: String,
    pub message: String,
    pub committer_date: String,
    pub committer_name: String,
    pub committer_email: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    pub fn as_git_flag(&self) -> &'static str {
        match self {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            ResetMode::Soft => "Soft",
            ResetMode::Mixed => "Mixed",
            ResetMode::Hard => "Hard",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetReflogOptions {
    pub target_ref: String,
    pub commit_sha: String,
    pub mode: ResetMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetOutcome {
    pub success: bool,
    pub restored_ref: String,
    pub restored_commit: String,
    pub restored_commit_short: String,
    pub mode: ResetMode,
    pub summary: String,
}

fn parse_reflog_operation_and_message(raw_subject: &str) -> (String, String) {
    let trimmed = raw_subject.trim();
    if let Some((op, msg)) = trimmed.split_once(':') {
        let op = op.trim();
        let msg = msg.trim();
        (op.to_string(), msg.to_string())
    } else {
        ("other".to_string(), trimmed.to_string())
    }
}

fn parse_selector_index(selector: &str) -> usize {
    if let Some(start) = selector.find("@{") {
        if let Some(end) = selector[start + 2..].find('}') {
            let num_str = &selector[start + 2..start + 2 + end];
            if let Ok(idx) = num_str.parse::<usize>() {
                return idx;
            }
        }
    }
    0
}

pub async fn query_reflog(
    layer: &ProcessLayer,
    repo_root: &Path,
    ref_target: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<ReflogEntry>, String> {
    let target = ref_target.unwrap_or("HEAD");
    let max_count = limit.unwrap_or(100);

    let format_arg = "--format=%gd%x00%h%x00%H%x00%gs%x00%ci%x00%an%x00%ae%x01".to_string();
    let count_arg = format!("-n{}", max_count);

    let call = GitCall::new(
        repo_root,
        vec![
            "reflog".to_string(),
            "show".to_string(),
            target.to_string(),
            count_arg,
            format_arg,
        ],
    );

    let res = layer
        .run(call, Intent::Read, CancellationToken::new())
        .await
        .map_err(|e| e.to_string())?;

    if !res.ok() {
        if res.stderr.contains("unknown revision")
            || res.stderr.contains("fatal: ambiguous argument")
            || res.stderr.contains("does not exist")
        {
            return Err(format!(
                "Reference '{}' does not exist in repository",
                target
            ));
        }
        return Ok(Vec::new());
    }

    let stdout = res.stdout_utf8_lossy();
    let mut entries = Vec::new();

    for block in stdout.split('\x01') {
        let trimmed_block = block.trim();
        if trimmed_block.is_empty() {
            continue;
        }

        let fields: Vec<&str> = trimmed_block.split('\x00').collect();
        if fields.len() >= 7 {
            let selector = fields[0].trim().to_string();
            let short_sha = fields[1].trim().to_string();
            let commit_sha = fields[2].trim().to_string();
            let raw_subject = fields[3];
            let committer_date = fields[4].trim().to_string();
            let committer_name = fields[5].trim().to_string();
            let committer_email = fields[6].trim().to_string();

            let (operation, message) = parse_reflog_operation_and_message(raw_subject);
            let index = parse_selector_index(&selector);

            entries.push(ReflogEntry {
                selector,
                index,
                short_sha,
                commit_sha,
                operation,
                message,
                committer_date,
                committer_name,
                committer_email,
            });
        }
    }

    Ok(entries)
}

pub async fn reset_to_reflog_entry(
    layer: &ProcessLayer,
    repo_root: &Path,
    options: &ResetReflogOptions,
) -> Result<ResetOutcome, String> {
    // 1. Verify target commit exists
    let verify_call = GitCall::new(
        repo_root,
        vec![
            "rev-parse".to_string(),
            "--verify".to_string(),
            options.commit_sha.clone(),
        ],
    );
    let verify_res = layer
        .run(verify_call, Intent::Read, CancellationToken::new())
        .await
        .map_err(|e| e.to_string())?;

    if !verify_res.ok() {
        return Err(format!(
            "Target commit '{}' does not exist",
            options.commit_sha
        ));
    }

    let full_commit = verify_res.stdout_utf8_lossy().trim().to_string();

    // 2. Get short sha and commit subject
    let short_call = GitCall::new(
        repo_root,
        vec![
            "rev-parse".to_string(),
            "--short".to_string(),
            full_commit.clone(),
        ],
    );
    let short_sha = layer
        .run(short_call, Intent::Read, CancellationToken::new())
        .await
        .map(|out| out.stdout_utf8_lossy().trim().to_string())
        .unwrap_or_else(|_| full_commit.chars().take(7).collect());

    let subject_call = GitCall::new(
        repo_root,
        vec![
            "log".to_string(),
            "-1".to_string(),
            "--format=%s".to_string(),
            full_commit.clone(),
        ],
    );
    let subject = layer
        .run(subject_call, Intent::Read, CancellationToken::new())
        .await
        .map(|out| out.stdout_utf8_lossy().trim().to_string())
        .unwrap_or_default();

    // 3. Find current branch
    let sym_call = GitCall::new(
        repo_root,
        vec![
            "symbolic-ref".to_string(),
            "--short".to_string(),
            "HEAD".to_string(),
        ],
    );
    let sym_res = layer
        .run(sym_call, Intent::Read, CancellationToken::new())
        .await
        .map_err(|e| e.to_string())?;

    let current_branch = if sym_res.ok() {
        sym_res.stdout_utf8_lossy().trim().to_string()
    } else {
        "HEAD".to_string()
    };

    let is_checked_out = options.target_ref == "HEAD" || options.target_ref == current_branch;

    if is_checked_out {
        // Run git reset with the requested mode
        let reset_call = GitCall::new(
            repo_root,
            vec![
                "reset".to_string(),
                options.mode.as_git_flag().to_string(),
                full_commit.clone(),
            ],
        );
        let reset_res = layer
            .run(reset_call, Intent::Write, CancellationToken::new())
            .await
            .map_err(|e| e.to_string())?;

        if !reset_res.ok() {
            return Err(format!("git reset failed: {}", reset_res.stderr.trim()));
        }
    } else {
        // Reset a branch that is not currently checked out
        let ref_path = if options.target_ref.starts_with("refs/") {
            options.target_ref.clone()
        } else {
            format!("refs/heads/{}", options.target_ref)
        };

        let update_msg = format!("reset: moving to {}", full_commit);
        let update_call = GitCall::new(
            repo_root,
            vec![
                "update-ref".to_string(),
                "-m".to_string(),
                update_msg,
                ref_path,
                full_commit.clone(),
            ],
        );
        let update_res = layer
            .run(update_call, Intent::Write, CancellationToken::new())
            .await
            .map_err(|e| e.to_string())?;

        if !update_res.ok() {
            return Err(format!(
                "Failed to update branch ref: {}",
                update_res.stderr.trim()
            ));
        }
    }

    let summary = format!(
        "Successfully reset '{}' ({}) to {} ({})",
        options.target_ref,
        options.mode.display_label(),
        short_sha,
        subject
    );

    Ok(ResetOutcome {
        success: true,
        restored_ref: options.target_ref.clone(),
        restored_commit: full_commit,
        restored_commit_short: short_sha,
        mode: options.mode,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::fs;

    async fn create_test_repo(layer: &ProcessLayer) -> TempDir {
        let dir = TempDir::new().unwrap();
        let path = dir.path();

        layer
            .run(
                GitCall::new(path, vec!["init", "-b", "master"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        layer
            .run(
                GitCall::new(path, vec!["config", "user.name", "Test"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        layer
            .run(
                GitCall::new(path, vec!["config", "user.email", "test@test.com"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        fs::write(path.join("file.txt"), "Initial\n").await.unwrap();
        layer
            .run(
                GitCall::new(path, vec!["add", "file.txt"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        layer
            .run(
                GitCall::new(path, vec!["commit", "-m", "Initial commit"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        fs::write(path.join("file.txt"), "Second edit\n")
            .await
            .unwrap();
        layer
            .run(
                GitCall::new(path, vec!["add", "file.txt"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        layer
            .run(
                GitCall::new(path, vec!["commit", "-m", "Second commit"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        dir
    }

    #[tokio::test]
    async fn test_query_reflog_head() {
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let repo = create_test_repo(&layer).await;
        let entries = query_reflog(&layer, repo.path(), Some("HEAD"), Some(10))
            .await
            .unwrap();

        assert!(entries.len() >= 2);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].operation, "commit");
        assert_eq!(entries[0].message, "Second commit");
        assert_eq!(entries[1].message, "Initial commit");
    }

    #[tokio::test]
    async fn test_query_reflog_branch() {
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let repo = create_test_repo(&layer).await;

        // Create and checkout feature branch
        layer
            .run(
                GitCall::new(repo.path(), vec!["checkout", "-b", "feature"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        fs::write(repo.path().join("feature.txt"), "Feature content\n")
            .await
            .unwrap();
        layer
            .run(
                GitCall::new(repo.path(), vec!["add", "feature.txt"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        layer
            .run(
                GitCall::new(repo.path(), vec!["commit", "-m", "Feature commit"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        let feature_entries = query_reflog(&layer, repo.path(), Some("feature"), None)
            .await
            .unwrap();
        assert!(!feature_entries.is_empty());
        assert_eq!(feature_entries[0].operation, "commit");
        assert_eq!(feature_entries[0].message, "Feature commit");
    }

    #[tokio::test]
    async fn test_undo_bad_hard_reset_via_reflog() {
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let repo = create_test_repo(&layer).await;
        let path = repo.path();

        // Check initial state has 2 commits
        let entries_before = query_reflog(&layer, path, Some("HEAD"), None)
            .await
            .unwrap();
        let second_commit_sha = entries_before[0].commit_sha.clone();
        let first_commit_sha = entries_before[1].commit_sha.clone();

        // User accidentally hard-resets to initial commit
        layer
            .run(
                GitCall::new(path, vec!["reset", "--hard", &first_commit_sha]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        // Now HEAD is at first commit, but reflog records the prior position
        let current_head = layer
            .run(
                GitCall::new(path, vec!["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(current_head.stdout_utf8_lossy().trim(), first_commit_sha);

        let reflog_after_bad_reset = query_reflog(&layer, path, Some("HEAD"), None)
            .await
            .unwrap();
        assert_eq!(reflog_after_bad_reset[0].operation, "reset");
        // Prior commit (second_commit_sha) is at HEAD@{1}
        assert_eq!(reflog_after_bad_reset[1].commit_sha, second_commit_sha);

        // Restore prior position in one confirmed action
        let outcome = reset_to_reflog_entry(
            &layer,
            path,
            &ResetReflogOptions {
                target_ref: "HEAD".to_string(),
                commit_sha: second_commit_sha.clone(),
                mode: ResetMode::Hard,
            },
        )
        .await
        .unwrap();

        assert!(outcome.success);
        let restored_head = layer
            .run(
                GitCall::new(path, vec!["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(restored_head.stdout_utf8_lossy().trim(), second_commit_sha);

        let file_content = fs::read_to_string(path.join("file.txt")).await.unwrap();
        assert_eq!(file_content, "Second edit\n");
    }

    #[tokio::test]
    async fn test_reset_non_checked_out_branch() {
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let repo = create_test_repo(&layer).await;
        let path = repo.path();

        // Create feature branch at second commit
        layer
            .run(
                GitCall::new(path, vec!["branch", "feature", "master"]),
                Intent::Write,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        // Get first commit sha
        let first_sha_out = layer
            .run(
                GitCall::new(path, vec!["rev-parse", "master~1"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let first_sha = first_sha_out.stdout_utf8_lossy().trim().to_string();

        // Reset 'feature' while staying on 'master'
        let outcome = reset_to_reflog_entry(
            &layer,
            path,
            &ResetReflogOptions {
                target_ref: "feature".to_string(),
                commit_sha: first_sha.clone(),
                mode: ResetMode::Mixed,
            },
        )
        .await
        .unwrap();

        assert!(outcome.success);

        // Verify feature points to first_sha
        let feature_rev = layer
            .run(
                GitCall::new(path, vec!["rev-parse", "feature"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(feature_rev.stdout_utf8_lossy().trim(), first_sha);

        // Verify master was untouched
        let master_rev = layer
            .run(
                GitCall::new(path, vec!["rev-parse", "master"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_ne!(master_rev.stdout_utf8_lossy().trim(), first_sha);
    }
}
