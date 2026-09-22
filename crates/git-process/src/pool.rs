use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::{Mutex as AsyncMutex, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::env::pinned_env;
use crate::log::OperationLog;
use crate::{GitCall, GitError, GitResult};

/// Whether a call is a background/automatic read, or an explicit user write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Read,
    Write,
}

/// Git subcommands allowed under `Intent::Read`; anything else is refused.
const READ_ALLOWLIST: &[&str] = &[
    "status",
    "log",
    "show",
    "diff",
    "diff-tree",
    "rev-parse",
    "rev-list",
    "ls-tree",
    "ls-files",
    "ls-remote",
    "branch",
    "tag",
    "stash",
    "remote",
    "config",
    "symbolic-ref",
    "for-each-ref",
    "cat-file",
    "blame",
    "check-ignore",
    "check-ref-format",
    "submodule",
    "reflog",
    "describe",
    "merge-base",
    "name-rev",
    "shortlog",
    "var",
    "version",
];
// `fetch` is deliberately absent: it writes, so it always requires `Intent::Write`.

/// Global flags that take a separate value argument, skipped when hunting for the subcommand token.
const GLOBAL_FLAGS_WITH_VALUE: &[&str] = &["-C", "-c", "--git-dir", "--work-tree", "--namespace"];

fn is_allowlisted(args: &[String]) -> bool {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--version" || arg == "-v" {
            return READ_ALLOWLIST.contains(&"version");
        }
        if GLOBAL_FLAGS_WITH_VALUE.contains(&arg.as_str()) {
            iter.next(); // skip the flag's value
            continue;
        }
        if arg.starts_with('-') {
            continue; // a boolean global flag, e.g. --no-pager
        }
        return READ_ALLOWLIST.contains(&arg.as_str());
    }
    false
}

pub struct ProcessLayer {
    semaphore: Arc<Semaphore>,
    write_locks: AsyncMutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>,
    pub log: Arc<OperationLog>,
    default_timeout: Duration,
}

impl ProcessLayer {
    pub fn new(concurrency: usize, default_timeout: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(concurrency.max(1))),
            write_locks: AsyncMutex::new(HashMap::new()),
            log: Arc::new(OperationLog::default()),
            default_timeout,
        }
    }

    async fn write_lock_for(&self, repo_root: &Path) -> Arc<AsyncMutex<()>> {
        let mut locks = self.write_locks.lock().await;
        locks
            .entry(repo_root.to_path_buf())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    pub async fn run(
        &self,
        call: GitCall,
        intent: Intent,
        cancel: CancellationToken,
    ) -> Result<GitResult, GitError> {
        if intent == Intent::Read && !is_allowlisted(&call.args) {
            return Err(GitError::NotAllowlisted(
                call.args.first().cloned().unwrap_or_default(),
            ));
        }

        // Serialize writes per repository; reads never wait on this lock.
        let repo_lock = if intent == Intent::Write {
            Some(self.write_lock_for(&call.repo_root).await)
        } else {
            None
        };
        let _write_guard = match &repo_lock {
            Some(lock) => Some(lock.lock().await),
            None => None,
        };

        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| GitError::Cancelled)?;

        let mut cmd = Command::new("git");
        cmd.args(&call.args)
            .current_dir(&call.repo_root)
            .stdin(if call.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in pinned_env(false) {
            cmd.env(k, v);
        }
        for (k, v) in &call.extra_env {
            cmd.env(k, v);
        }

        let started = Instant::now();
        let mut child = cmd.spawn()?;

        if let Some(input) = &call.stdin {
            use tokio::io::AsyncWriteExt;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(input).await?;
            }
        }

        let wait = child.wait_with_output();
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                Err(GitError::Cancelled)
            }
            _ = tokio::time::sleep(self.default_timeout) => {
                Err(GitError::Timeout(self.default_timeout))
            }
            out = wait => {
                out.map_err(GitError::Spawn)
            }
        };

        let duration = started.elapsed();

        match result {
            Ok(output) => {
                let status = output.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                self.log.record(
                    call.repo_root.clone(),
                    call.args.clone(),
                    status,
                    duration,
                    intent == Intent::Write,
                    stderr.clone(),
                );
                Ok(GitResult {
                    status,
                    stdout: output.stdout,
                    stderr,
                    duration,
                })
            }
            Err(err) => {
                let err_str = err.to_string();
                self.log.record(
                    call.repo_root.clone(),
                    call.args.clone(),
                    -1,
                    duration,
                    intent == Intent::Write,
                    err_str,
                );
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_allowlist_accepts_known_subcommands() {
        assert!(is_allowlisted(&["status".into(), "--porcelain=v2".into()]));
        assert!(is_allowlisted(&["-C".into(), "/tmp".into(), "log".into()]));
    }

    #[test]
    fn read_allowlist_rejects_writes() {
        assert!(!is_allowlisted(&["commit".into(), "-m".into(), "x".into()]));
        assert!(!is_allowlisted(&["push".into()]));
        assert!(!is_allowlisted(&["reset".into(), "--hard".into()]));
    }

    #[tokio::test]
    async fn refuses_non_allowlisted_read_before_spawning() {
        let layer = ProcessLayer::new(2, Duration::from_secs(5));
        let call = GitCall::new("/tmp", ["commit", "-m", "should not run"]);
        let result = layer
            .run(call, Intent::Read, CancellationToken::new())
            .await;
        assert!(matches!(result, Err(GitError::NotAllowlisted(_))));
        assert!(
            layer.log.is_empty(),
            "refused calls must not spawn a process or be logged"
        );
    }

    #[tokio::test]
    async fn runs_a_real_allowlisted_command() {
        let layer = ProcessLayer::new(2, Duration::from_secs(5));
        let call = GitCall::new(".", ["--version"]);
        let result = layer
            .run(call, Intent::Read, CancellationToken::new())
            .await
            .unwrap();
        assert!(result.ok());
        assert!(result.stdout_utf8_lossy().contains("git version"));
        assert_eq!(layer.log.len(), 1);
    }

    #[tokio::test]
    async fn write_intent_bypasses_allowlist_but_is_logged_as_write() {
        let layer = ProcessLayer::new(2, Duration::from_secs(5));
        let call = GitCall::new(".", ["--version"]);
        layer
            .run(call, Intent::Write, CancellationToken::new())
            .await
            .unwrap();
        let snapshot = layer.log.snapshot();
        assert!(snapshot[0].write);
    }

    #[tokio::test]
    async fn stdin_is_piped_to_the_child_process() {
        let layer = ProcessLayer::new(2, Duration::from_secs(5));
        let call = GitCall::new(".", ["hash-object", "--stdin"]).with_stdin(b"hello\n".to_vec());
        let result = layer
            .run(call, Intent::Write, CancellationToken::new())
            .await
            .unwrap();
        assert!(result.ok());
        assert_eq!(
            result.stdout_utf8_lossy().trim(),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
    }

    #[tokio::test]
    async fn cancellation_stops_the_call() {
        let layer = ProcessLayer::new(2, Duration::from_secs(30));
        let token = CancellationToken::new();
        token.cancel();
        let call = GitCall::new(".", ["--version"]);
        let result = layer.run(call, Intent::Read, token).await;
        assert!(matches!(result, Err(GitError::Cancelled)));
    }

    #[tokio::test]
    async fn test_no_non_zero_exit_is_ever_reported_as_success() {
        let layer = ProcessLayer::new(2, Duration::from_secs(5));
        // A git command that fails with non-zero exit
        let call = GitCall::new(".", ["log", "nonexistent-ref-surely-missing-4242"]);
        let result = layer
            .run(call, Intent::Read, CancellationToken::new())
            .await
            .expect("process layer returned error instead of GitResult");

        // Assert: no non-zero exit is reported as ok() or success
        assert!(!result.ok(), "A non-zero exit status must NEVER be reported as ok()");
        assert_ne!(result.status, 0, "Exit status must be non-zero");
        assert!(
            !result.stderr.is_empty(),
            "Stderr must not be empty on command failure"
        );
        assert!(
            result.stderr.to_lowercase().contains("fatal")
                || result.stderr.to_lowercase().contains("error"),
            "Stderr must expose the verbatim git error message"
        );

        // Verify operation log recorded verbatim args, exit status and stderr
        let snapshot = layer.log.snapshot();
        let last = snapshot.last().expect("must have logged entry");
        assert_eq!(last.args, vec!["log", "nonexistent-ref-surely-missing-4242"]);
        assert_eq!(last.exit_status, result.status);
        assert_eq!(last.stderr, result.stderr);
    }
}
