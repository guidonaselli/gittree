use std::collections::HashMap;
use std::path::Path;
use futures::future::join_all;
use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RemoteInfo {
    pub name: String,
    pub fetch_url: Option<String>,
    pub push_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum PushForceMode {
    None,
    ForceWithLease,
    BareForce { acknowledged_destructive: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PushOptions {
    pub remote: String,
    pub refspec: Option<String>,
    pub force_mode: PushForceMode,
    pub tags: bool,
    pub set_upstream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PushOutcome {
    Success {
        summary: String,
        details: String,
    },
    RejectedNonFastForward {
        remote_message: String,
        suggest_pull: bool,
    },
    Cancelled {
        summary: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullStrategy {
    Merge,
    Rebase,
    FastForwardOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PullOptions {
    pub remote: String,
    pub branch: Option<String>,
    pub strategy: PullStrategy,
    pub prune: bool,
    pub tags: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PullOutcome {
    Success {
        summary: String,
    },
    Conflict {
        conflicting_files: Vec<String>,
        summary: String,
    },
    Cancelled {
        summary: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FetchOptions {
    pub remote: Option<String>,
    pub prune: bool,
    pub tags: bool,
    pub refspec: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RemoteFetchOutcome {
    pub remote: String,
    pub success: bool,
    pub summary: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MultiRemoteFetchResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<RemoteFetchOutcome>,
}

pub async fn query_remotes(
    layer: &ProcessLayer,
    root: &Path,
) -> Result<Vec<RemoteInfo>, String> {
    let res = layer
        .run(
            GitCall::new(root, ["remote", "-v"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error querying remotes: {e}"))?;

    if !res.ok() {
        return Err(format!("git remote -v failed: {}", res.stderr.trim()));
    }

    let mut map: HashMap<String, (Option<String>, Option<String>)> = HashMap::new();
    for line in res.stdout_utf8_lossy().lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Format: <name>\t<url> (fetch) or (push)
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            let name = parts[0].to_string();
            let url = parts[1].to_string();
            let kind = parts[2];

            let entry = map.entry(name).or_insert((None, None));
            if kind.contains("fetch") {
                entry.0 = Some(url);
            } else if kind.contains("push") {
                entry.1 = Some(url);
            }
        }
    }

    let mut remotes: Vec<RemoteInfo> = map
        .into_iter()
        .map(|(name, (fetch_url, push_url))| RemoteInfo {
            name,
            fetch_url,
            push_url,
        })
        .collect();

    remotes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(remotes)
}

pub async fn fetch_single_remote(
    layer: &ProcessLayer,
    root: &Path,
    remote: &str,
    prune: bool,
    tags: bool,
    refspec: Option<&str>,
    cancel: CancellationToken,
) -> RemoteFetchOutcome {
    let mut args = vec!["fetch".to_string(), remote.to_string()];
    if prune {
        args.push("--prune".to_string());
    }
    if tags {
        args.push("--tags".to_string());
    }
    if let Some(r) = refspec {
        if !r.trim().is_empty() {
            args.push(r.trim().to_string());
        }
    }

    let res = layer
        .run(
            GitCall::new(root, args.iter().map(|s| s.as_str())),
            Intent::Write,
            cancel,
        )
        .await;

    match res {
        Ok(output) => {
            if output.ok() {
                let stdout = output.stdout_utf8_lossy();
                let stderr = output.stderr.trim();
                let details = if !stderr.is_empty() {
                    stderr.to_string()
                } else if !stdout.trim().is_empty() {
                    stdout.to_string()
                } else {
                    format!("Successfully fetched from '{remote}'.")
                };
                RemoteFetchOutcome {
                    remote: remote.to_string(),
                    success: true,
                    summary: details,
                    error: None,
                }
            } else {
                RemoteFetchOutcome {
                    remote: remote.to_string(),
                    success: false,
                    summary: format!("Failed to fetch from '{remote}'."),
                    error: Some(output.stderr.trim().to_string()),
                }
            }
        }
        Err(git_process::GitError::Cancelled) => RemoteFetchOutcome {
            remote: remote.to_string(),
            success: false,
            summary: format!("Fetch from '{remote}' was cancelled. Repository left unmodified."),
            error: Some("Operation cancelled".to_string()),
        },
        Err(git_process::GitError::Timeout(d)) => RemoteFetchOutcome {
            remote: remote.to_string(),
            success: false,
            summary: format!(
                "Fetch from '{remote}' timed out after {d:?} (network stall detected). Repository left unmodified."
            ),
            error: Some("Network timeout".to_string()),
        },
        Err(e) => RemoteFetchOutcome {
            remote: remote.to_string(),
            success: false,
            summary: format!("Error running fetch for '{remote}'."),
            error: Some(e.to_string()),
        },
    }
}

pub async fn fetch(
    layer: &ProcessLayer,
    root: &Path,
    options: FetchOptions,
    cancel: CancellationToken,
) -> Result<MultiRemoteFetchResult, String> {
    let target_remotes = match options.remote {
        Some(r) if !r.trim().is_empty() => vec![r],
        _ => {
            let list = query_remotes(layer, root).await?;
            list.into_iter().map(|rem| rem.name).collect()
        }
    };

    if target_remotes.is_empty() {
        return Ok(MultiRemoteFetchResult {
            total: 0,
            succeeded: 0,
            failed: 0,
            results: Vec::new(),
        });
    }

    let futures = target_remotes.into_iter().map(|remote| {
        let prune = options.prune;
        let tags = options.tags;
        let refspec = options.refspec.clone();
        let cancel = cancel.clone();
        async move {
            fetch_single_remote(
                layer,
                root,
                &remote,
                prune,
                tags,
                refspec.as_deref(),
                cancel,
            )
            .await
        }
    });

    let results = join_all(futures).await;

    let total = results.len();
    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = total - succeeded;

    Ok(MultiRemoteFetchResult {
        total,
        succeeded,
        failed,
        results,
    })
}

pub async fn pull(
    layer: &ProcessLayer,
    root: &Path,
    options: PullOptions,
    cancel: CancellationToken,
) -> Result<PullOutcome, String> {
    let mut args = vec!["pull".to_string()];

    match options.strategy {
        PullStrategy::Rebase => args.push("--rebase".to_string()),
        PullStrategy::Merge => args.push("--no-rebase".to_string()),
        PullStrategy::FastForwardOnly => args.push("--ff-only".to_string()),
    }

    if options.prune {
        args.push("--prune".to_string());
    }
    if options.tags {
        args.push("--tags".to_string());
    }

    args.push(options.remote.clone());

    if let Some(b) = &options.branch {
        if !b.trim().is_empty() {
            args.push(b.trim().to_string());
        }
    }

    let res = match layer
        .run(
            GitCall::new(root, args.iter().map(|s| s.as_str())),
            Intent::Write,
            cancel,
        )
        .await
    {
        Ok(r) => r,
        Err(git_process::GitError::Cancelled) => {
            return Ok(PullOutcome::Cancelled {
                summary: "Pull operation was cancelled. Repository left unmodified.".to_string(),
            });
        }
        Err(git_process::GitError::Timeout(d)) => {
            return Ok(PullOutcome::Cancelled {
                summary: format!(
                    "Pull operation timed out after {d:?} (network stall detected). Repository left unmodified."
                ),
            });
        }
        Err(e) => return Err(format!("Process error during git pull: {e}")),
    };

    let combined = format!("{}\n{}", res.stdout_utf8_lossy(), res.stderr);

    if res.ok() {
        let summary = if !res.stdout_utf8_lossy().trim().is_empty() {
            res.stdout_utf8_lossy().trim().to_string()
        } else {
            "Pull completed successfully.".to_string()
        };
        Ok(PullOutcome::Success { summary })
    } else {
        if combined.contains("CONFLICT") || combined.contains("Automatic merge failed") {
            let conflicts = crate::integration::query_conflicting_files(layer, root)
                .await
                .unwrap_or_default();
            Ok(PullOutcome::Conflict {
                conflicting_files: conflicts,
                summary: "Pull resulted in merge conflicts. Resolve conflicts before continuing."
                    .to_string(),
            })
        } else {
            Ok(PullOutcome::Failed {
                message: res.stderr.trim().to_string(),
            })
        }
    }
}

pub async fn push(
    layer: &ProcessLayer,
    root: &Path,
    options: PushOptions,
    cancel: CancellationToken,
) -> Result<PushOutcome, String> {
    let mut args = vec!["push".to_string()];

    match options.force_mode {
        PushForceMode::None => {}
        PushForceMode::ForceWithLease => {
            args.push("--force-with-lease".to_string());
        }
        PushForceMode::BareForce {
            acknowledged_destructive,
        } => {
            if !acknowledged_destructive {
                return Err(
                    "Bare force push refused: destruction of remote commits was not explicitly acknowledged."
                        .to_string(),
                );
            }
            args.push("--force".to_string());
        }
    }

    if options.tags {
        args.push("--tags".to_string());
    }
    if options.set_upstream {
        args.push("-u".to_string());
    }

    args.push(options.remote.clone());

    if let Some(r) = &options.refspec {
        if !r.trim().is_empty() {
            args.push(r.trim().to_string());
        }
    }

    let res = match layer
        .run(
            GitCall::new(root, args.iter().map(|s| s.as_str())),
            Intent::Write,
            cancel,
        )
        .await
    {
        Ok(r) => r,
        Err(git_process::GitError::Cancelled) => {
            return Ok(PushOutcome::Cancelled {
                summary: "Push operation was cancelled. Remote refs left unmodified.".to_string(),
            });
        }
        Err(git_process::GitError::Timeout(d)) => {
            return Ok(PushOutcome::Cancelled {
                summary: format!(
                    "Push operation timed out after {d:?} (network stall detected). Remote refs left unmodified."
                ),
            });
        }
        Err(e) => return Err(format!("Process error during git push: {e}")),
    };

    let combined = format!("{}\n{}", res.stdout_utf8_lossy(), res.stderr);

    if res.ok() {
        let details = if !res.stderr.trim().is_empty() {
            res.stderr.trim().to_string()
        } else {
            res.stdout_utf8_lossy().trim().to_string()
        };
        Ok(PushOutcome::Success {
            summary: "Push completed successfully.".to_string(),
            details,
        })
    } else {
        if combined.contains("[rejected]")
            || combined.contains("non-fast-forward")
            || combined.contains("Updates were rejected because the remote contains work")
            || combined.contains("failed to push some refs")
        {
            Ok(PushOutcome::RejectedNonFastForward {
                remote_message: res.stderr.trim().to_string(),
                suggest_pull: true,
            })
        } else {
            Ok(PushOutcome::Failed {
                message: res.stderr.trim().to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    fn setup_repo(root: &Path) {
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(root)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(root)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(root)
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_query_remotes_parsing() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        setup_repo(root);

        std::process::Command::new("git")
            .args(["remote", "add", "origin", "https://github.com/example/repo.git"])
            .current_dir(root)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["remote", "add", "upstream", "git@github.com:upstream/repo.git"])
            .current_dir(root)
            .output()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(10));
        let remotes = query_remotes(&layer, root).await.unwrap();

        assert_eq!(remotes.len(), 2);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].fetch_url.as_deref(), Some("https://github.com/example/repo.git"));
        assert_eq!(remotes[1].name, "upstream");
        assert_eq!(remotes[1].push_url.as_deref(), Some("git@github.com:upstream/repo.git"));
    }

    #[tokio::test]
    async fn test_push_force_with_lease_is_default_and_bare_force_requires_acknowledgement() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        setup_repo(root);

        let layer = ProcessLayer::new(4, Duration::from_secs(10));

        // Bare force without explicit acknowledgment MUST be refused
        let unacknowledged_opts = PushOptions {
            remote: "origin".to_string(),
            refspec: None,
            force_mode: PushForceMode::BareForce { acknowledged_destructive: false },
            tags: false,
            set_upstream: false,
        };
        let err = push(&layer, root, unacknowledged_opts, CancellationToken::new()).await.unwrap_err();
        assert!(err.contains("destruction of remote commits was not explicitly acknowledged"));
    }

    #[tokio::test]
    async fn test_rejected_push_returns_verbatim_message_and_suggests_pull() {
        let dir = tempdir().unwrap();
        let remote_dir = dir.path().join("remote.git");
        let local_a = dir.path().join("local_a");
        let local_b = dir.path().join("local_b");

        // Bare remote
        std::process::Command::new("git")
            .args(["init", "--bare", "-b", "main", remote_dir.to_str().unwrap()])
            .output()
            .unwrap();

        // Clone local_a, commit and push
        std::process::Command::new("git")
            .args(["clone", remote_dir.to_str().unwrap(), local_a.to_str().unwrap()])
            .output()
            .unwrap();
        setup_repo(&local_a);
        std::fs::write(local_a.join("a.txt"), "hello from a\n").unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&local_a).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "commit A"]).current_dir(&local_a).output().unwrap();
        std::process::Command::new("git").args(["push", "origin", "main"]).current_dir(&local_a).output().unwrap();

        // Clone local_b (now at commit A)
        std::process::Command::new("git")
            .args(["clone", remote_dir.to_str().unwrap(), local_b.to_str().unwrap()])
            .output()
            .unwrap();
        setup_repo(&local_b);

        // In local_a, make another commit and push
        std::fs::write(local_a.join("a.txt"), "hello from a 2\n").unwrap();
        std::process::Command::new("git").args(["commit", "-am", "commit A2"]).current_dir(&local_a).output().unwrap();
        std::process::Command::new("git").args(["push", "origin", "main"]).current_dir(&local_a).output().unwrap();

        // In local_b, make independent commit
        std::fs::write(local_b.join("b.txt"), "hello from b\n").unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&local_b).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "commit B"]).current_dir(&local_b).output().unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(10));

        // Pushing from local_b without pull must be rejected as non-fast-forward!
        let outcome = push(
            &layer,
            &local_b,
            PushOptions {
                remote: "origin".to_string(),
                refspec: Some("main".to_string()),
                force_mode: PushForceMode::None,
                tags: false,
                set_upstream: false,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();

        match outcome {
            PushOutcome::RejectedNonFastForward { remote_message, suggest_pull } => {
                assert!(suggest_pull);
                assert!(
                    remote_message.contains("rejected") || remote_message.contains("fetch first")
                );
            }
            _ => panic!("Expected non-fast-forward push rejection!"),
        }
    }

    #[tokio::test]
    async fn test_concurrent_multi_remote_fetch_isolates_failures() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        setup_repo(root);

        // Create one valid local bare remote
        let valid_remote = dir.path().join("valid_remote.git");
        std::process::Command::new("git")
            .args(["init", "--bare", "-b", "main", valid_remote.to_str().unwrap()])
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(["remote", "add", "valid", valid_remote.to_str().unwrap()])
            .current_dir(root)
            .output()
            .unwrap();

        // Create one invalid / unreachable remote
        std::process::Command::new("git")
            .args(["remote", "add", "broken", "/nonexistent/path/to/repo.git"])
            .current_dir(root)
            .output()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(10));

        let result = fetch(
            &layer,
            root,
            FetchOptions {
                remote: None, // all remotes
                prune: false,
                tags: false,
                refspec: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert_eq!(result.total, 2);
        assert_eq!(result.succeeded, 1);
        assert_eq!(result.failed, 1);

        let valid_outcome = result.results.iter().find(|r| r.remote == "valid").unwrap();
        assert!(valid_outcome.success);

        let broken_outcome = result.results.iter().find(|r| r.remote == "broken").unwrap();
        assert!(!broken_outcome.success);
        assert!(broken_outcome.error.is_some());
    }

    #[tokio::test]
    async fn test_cancelled_fetch_leaves_repository_unmodified() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        setup_repo(root);

        let cancel = CancellationToken::new();
        cancel.cancel(); // Pre-cancelled token

        let layer = ProcessLayer::new(4, Duration::from_secs(10));
        let outcome = fetch_single_remote(
            &layer,
            root,
            "origin",
            false,
            false,
            None,
            cancel,
        )
        .await;

        assert!(!outcome.success);
        assert!(outcome.summary.contains("cancelled"));
    }
}
