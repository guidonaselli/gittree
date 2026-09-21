use git_process::{GitCall, Intent, ProcessLayer};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StashEntry {
    pub index: usize,
    pub selector: String,
    pub commit_sha: String,
    pub short_sha: String,
    pub message: String,
    pub date: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StashFileStat {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StashDetail {
    pub entry: StashEntry,
    pub diff: String,
    pub changed_files: Vec<StashFileStat>,
    pub untracked_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateStashOptions {
    pub message: Option<String>,
    pub include_untracked: bool,
    pub keep_index: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum StashApplyOutcome {
    Clean,
    Conflict {
        conflicting_files: Vec<String>,
        message: String,
        stash_retained: bool,
    },
}

pub async fn query_stashes(layer: &ProcessLayer, root: &Path) -> Result<Vec<StashEntry>, String> {
    let fmt = "%gd%x00%h%x00%H%x00%gs%x00%cI%x01";
    let res = layer
        .run(
            GitCall::new(root, ["stash", "list", &format!("--format={fmt}")]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error querying stashes: {e}"))?;

    if !res.ok() {
        return Err(format!("git stash list failed: {}", res.stderr.trim()));
    }

    let mut stashes = Vec::new();
    let stdout = res.stdout_utf8_lossy();
    let records = stdout.split('\x01');

    for rec in records {
        let trimmed = rec.trim_matches('\n');
        if trimmed.is_empty() {
            continue;
        }

        let fields: Vec<&str> = trimmed.split('\0').collect();
        if fields.len() < 5 {
            continue;
        }

        let selector = fields[0].to_string(); // e.g. "stash@{0}"
        let index = selector
            .strip_prefix("stash@{")
            .and_then(|s| s.strip_suffix('}'))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(stashes.len());

        let short_sha = fields[1].to_string();
        let commit_sha = fields[2].to_string();
        let message = fields[3].to_string();
        let date = fields[4].to_string();

        let branch = parse_branch_from_stash_msg(&message);

        stashes.push(StashEntry {
            index,
            selector,
            commit_sha,
            short_sha,
            message,
            date,
            branch,
        });
    }

    Ok(stashes)
}

fn parse_branch_from_stash_msg(msg: &str) -> Option<String> {
    if let Some(rest) = msg.strip_prefix("WIP on ") {
        let branch_part = rest.split(':').next()?.trim();
        return Some(branch_part.to_string());
    }
    if let Some(rest) = msg.strip_prefix("On ") {
        let branch_part = rest.split(':').next()?.trim();
        return Some(branch_part.to_string());
    }
    None
}

pub async fn inspect_stash(
    layer: &ProcessLayer,
    root: &Path,
    selector: &str,
) -> Result<StashDetail, String> {
    let all = query_stashes(layer, root).await?;
    let entry = all
        .into_iter()
        .find(|s| s.selector == selector)
        .ok_or_else(|| format!("Stash '{selector}' not found"))?;

    // 1. Unified diff against parent 1
    let parent1 = format!("{selector}^1");
    let diff_res = layer
        .run(
            GitCall::new(root, ["diff", "-p", &parent1, selector]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error getting stash diff: {e}"))?;

    let mut diff = if diff_res.ok() {
        diff_res.stdout_utf8_lossy().into_owned()
    } else {
        let fallback = layer
            .run(
                GitCall::new(root, ["stash", "show", "-p", selector]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .map_err(|e| format!("Process error running stash show: {e}"))?;
        fallback.stdout_utf8_lossy().into_owned()
    };

    // 2. Numstat
    let numstat_res = layer
        .run(
            GitCall::new(root, ["diff", "--numstat", &parent1, selector]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error getting stash numstat: {e}"))?;

    let mut changed_files = Vec::new();
    if numstat_res.ok() {
        let numstat_out = numstat_res.stdout_utf8_lossy();
        for line in numstat_out.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let adds = parts[0].parse::<u32>().unwrap_or(0);
                let dels = parts[1].parse::<u32>().unwrap_or(0);
                let path = parts[2..].join(" ");
                changed_files.push(StashFileStat {
                    path,
                    additions: adds,
                    deletions: dels,
                });
            }
        }
    }

    // 3. Untracked files (parent 3 of stash commit)
    let parent3 = format!("{selector}^3");
    let has_parent3 = layer
        .run(
            GitCall::new(root, ["rev-parse", "--verify", "--quiet", &parent3]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map(|r| r.ok())
        .unwrap_or(false);

    let mut untracked_files = Vec::new();
    if has_parent3 {
        let untracked_res = layer
            .run(
                GitCall::new(root, ["ls-tree", "-r", "--name-only", &parent3]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .map_err(|e| format!("Process error listing untracked stash files: {e}"))?;

        if untracked_res.ok() {
            let u_out = untracked_res.stdout_utf8_lossy();
            for line in u_out.lines() {
                let p = line.trim();
                if !p.is_empty() {
                    untracked_files.push(p.to_string());
                }
            }
        }

        // Append untracked diff if available
        let u_diff_res = layer
            .run(
                GitCall::new(root, ["show", "--format=", &parent3]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await;
        if let Ok(u_res) = u_diff_res {
            let u_stdout = u_res.stdout_utf8_lossy();
            if u_res.ok() && !u_stdout.is_empty() {
                diff.push_str("\n\n# Untracked files:\n");
                diff.push_str(&u_stdout);
            }
        }
    }

    Ok(StashDetail {
        entry,
        diff,
        changed_files,
        untracked_files,
    })
}

pub async fn create_stash(
    layer: &ProcessLayer,
    root: &Path,
    opts: CreateStashOptions,
) -> Result<String, String> {
    let mut args = vec!["stash".to_string(), "push".to_string()];
    if opts.include_untracked {
        args.push("-u".to_string());
    }
    if opts.keep_index {
        args.push("--keep-index".to_string());
    }
    if let Some(msg) = opts.message.as_ref().filter(|m| !m.trim().is_empty()) {
        args.push("-m".to_string());
        args.push(msg.trim().to_string());
    }

    let res = layer
        .run(GitCall::new(root, args), Intent::Write, CancellationToken::new())
        .await
        .map_err(|e| format!("Process error creating stash: {e}"))?;

    if !res.ok() {
        return Err(format!("git stash push failed: {}", res.stderr.trim()));
    }

    let stdout = res.stdout_utf8_lossy().trim().to_string();
    if stdout.contains("No local changes to save") {
        return Err("No local changes to save".to_string());
    }

    Ok(stdout)
}

pub async fn apply_stash(
    layer: &ProcessLayer,
    root: &Path,
    selector: &str,
    reinstate_index: bool,
) -> Result<StashApplyOutcome, String> {
    let mut args = vec!["stash".to_string(), "apply".to_string()];
    if reinstate_index {
        args.push("--index".to_string());
    }
    args.push(selector.to_string());

    let res = layer
        .run(GitCall::new(root, args), Intent::Write, CancellationToken::new())
        .await
        .map_err(|e| format!("Process error applying stash: {e}"))?;

    if res.ok() {
        return Ok(StashApplyOutcome::Clean);
    }

    let combined = format!("{}\n{}", res.stdout_utf8_lossy(), res.stderr);
    let conflicting_files = parse_conflict_files(&combined);

    Ok(StashApplyOutcome::Conflict {
        conflicting_files,
        message: combined.trim().to_string(),
        stash_retained: true,
    })
}

pub async fn pop_stash(
    layer: &ProcessLayer,
    root: &Path,
    selector: &str,
    reinstate_index: bool,
) -> Result<StashApplyOutcome, String> {
    let mut args = vec!["stash".to_string(), "pop".to_string()];
    if reinstate_index {
        args.push("--index".to_string());
    }
    args.push(selector.to_string());

    let res = layer
        .run(GitCall::new(root, args), Intent::Write, CancellationToken::new())
        .await
        .map_err(|e| format!("Process error popping stash: {e}"))?;

    if res.ok() {
        return Ok(StashApplyOutcome::Clean);
    }

    let combined = format!("{}\n{}", res.stdout_utf8_lossy(), res.stderr);
    let conflicting_files = parse_conflict_files(&combined);

    Ok(StashApplyOutcome::Conflict {
        conflicting_files,
        message: combined.trim().to_string(),
        stash_retained: true,
    })
}

pub async fn drop_stash(layer: &ProcessLayer, root: &Path, selector: &str) -> Result<(), String> {
    let res = layer
        .run(
            GitCall::new(root, ["stash", "drop", selector]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error dropping stash: {e}"))?;

    if !res.ok() {
        return Err(format!("git stash drop failed: {}", res.stderr.trim()));
    }

    Ok(())
}

pub async fn clear_stashes(layer: &ProcessLayer, root: &Path) -> Result<(), String> {
    let res = layer
        .run(
            GitCall::new(root, ["stash", "clear"]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error clearing stashes: {e}"))?;

    if !res.ok() {
        return Err(format!("git stash clear failed: {}", res.stderr.trim()));
    }

    Ok(())
}

fn parse_conflict_files(output: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut in_unmerged = false;
    let mut in_overwritten = false;

    for line in output.lines() {
        let trimmed = line.trim();

        // 1. CONFLICT (content): Merge conflict in <file>
        if let Some(pos) = line.find("CONFLICT (content): Merge conflict in ") {
            let rest = line[pos + 38..].trim();
            if !rest.is_empty() && !files.contains(&rest.to_string()) {
                files.push(rest.to_string());
            }
        } else if let Some(pos) = line.find("CONFLICT (modify/delete): ") {
            let rest = line[pos + 26..].trim();
            let file = rest.split_whitespace().next().unwrap_or(rest);
            if !file.is_empty() && !files.contains(&file.to_string()) {
                files.push(file.to_string());
            }
        }

        // 2. Unmerged paths section
        if line.contains("Unmerged paths:") {
            in_unmerged = true;
            in_overwritten = false;
            continue;
        }

        // 3. Overwritten files section
        if line.contains("Your local changes to the following files would be overwritten") {
            in_overwritten = true;
            in_unmerged = false;
            continue;
        }

        if in_unmerged {
            if line.starts_with('\t') || line.starts_with("        ") {
                if let Some(pos) = line.find(':') {
                    let path = line[pos + 1..].trim();
                    if !path.is_empty() && !files.contains(&path.to_string()) {
                        files.push(path.to_string());
                    }
                }
            } else if !trimmed.is_empty() && !trimmed.starts_with('(') {
                in_unmerged = false;
            }
        }

        if in_overwritten {
            if line.starts_with('\t') || line.starts_with("        ") {
                let path = trimmed;
                if !path.is_empty() && !files.contains(&path.to_string()) {
                    files.push(path.to_string());
                }
            } else if !trimmed.is_empty() && !trimmed.starts_with("Please commit") {
                in_overwritten = false;
            }
        }
    }

    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_process::ProcessLayer;
    use std::fs;
    use std::time::Duration;
    use tempfile::tempdir;

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_create_list_inspect_and_drop_stash() {
        let dir = tempdir().unwrap();
        git(dir.path(), &["init"]);
        git(dir.path(), &["config", "user.name", "Tester"]);
        git(dir.path(), &["config", "user.email", "tester@example.com"]);

        let tracked = dir.path().join("tracked.txt");
        fs::write(&tracked, "line 1\nline 2\n").unwrap();
        git(dir.path(), &["add", "tracked.txt"]);
        git(dir.path(), &["commit", "-m", "Initial commit"]);

        // Modify tracked file and add an untracked file
        fs::write(&tracked, "line 1\nline 2 modified\n").unwrap();
        let untracked = dir.path().join("untracked.txt");
        fs::write(&untracked, "new untracked file content\n").unwrap();

        let layer = ProcessLayer::new(10, Duration::from_secs(30));

        // 1. Create stash including untracked
        let create_msg = create_stash(
            &layer,
            dir.path(),
            CreateStashOptions {
                message: Some("WIP feature test".to_string()),
                include_untracked: true,
                keep_index: false,
            },
        )
        .await
        .unwrap();

        assert!(!create_msg.is_empty());
        assert!(!fs::read_to_string(&tracked).unwrap().contains("modified"));
        assert!(!untracked.exists());

        // 2. List stashes
        let stashes = query_stashes(&layer, dir.path()).await.unwrap();
        assert_eq!(stashes.len(), 1);
        assert_eq!(stashes[0].selector, "stash@{0}");
        assert!(stashes[0].message.contains("WIP feature test"));

        // 3. Inspect stash with diff before applying
        let detail = inspect_stash(&layer, dir.path(), "stash@{0}")
            .await
            .unwrap();

        assert_eq!(detail.entry.selector, "stash@{0}");
        assert!(detail.diff.contains("line 2 modified"));
        assert_eq!(detail.changed_files.len(), 1);
        assert_eq!(detail.changed_files[0].path, "tracked.txt");
        assert_eq!(detail.untracked_files, vec!["untracked.txt".to_string()]);

        // 4. Drop stash
        drop_stash(&layer, dir.path(), "stash@{0}").await.unwrap();
        let remaining = query_stashes(&layer, dir.path()).await.unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_stash_apply_and_pop_conflict_retains_entry() {
        let dir = tempdir().unwrap();
        git(dir.path(), &["init"]);
        git(dir.path(), &["config", "user.name", "Tester"]);
        git(dir.path(), &["config", "user.email", "tester@example.com"]);

        let file = dir.path().join("f.txt");
        fs::write(&file, "initial\n").unwrap();
        git(dir.path(), &["add", "f.txt"]);
        git(dir.path(), &["commit", "-m", "Commit 1"]);

        // Stash an edit
        fs::write(&file, "stashed version\n").unwrap();
        let layer = ProcessLayer::new(10, Duration::from_secs(30));
        create_stash(
            &layer,
            dir.path(),
            CreateStashOptions {
                message: Some("Stashed change".to_string()),
                include_untracked: false,
                keep_index: false,
            },
        )
        .await
        .unwrap();

        // Make conflicting commit in repo
        fs::write(&file, "conflicting commit version\n").unwrap();
        git(dir.path(), &["commit", "-am", "Commit 2"]);

        // Try pop: must fail with conflict and RETAIN the stash entry!
        let pop_outcome = pop_stash(&layer, dir.path(), "stash@{0}", false)
            .await
            .unwrap();

        match pop_outcome {
            StashApplyOutcome::Clean => panic!("Expected conflict, got clean"),
            StashApplyOutcome::Conflict {
                conflicting_files,
                stash_retained,
                ..
            } => {
                assert!(stash_retained, "Stash must be retained on conflict");
                assert_eq!(conflicting_files, vec!["f.txt".to_string()]);
            }
        }

        // Confirm the stash entry is STILL in the stash list
        let stashes = query_stashes(&layer, dir.path()).await.unwrap();
        assert_eq!(stashes.len(), 1, "Stash must be retained after pop conflict");
        assert_eq!(stashes[0].selector, "stash@{0}");
    }
}
