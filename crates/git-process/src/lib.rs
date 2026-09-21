//! Process layer: the only place GitTree talks to `git`. Argv-only, no shell,
//! pinned environment, bounded concurrency, per-repository write serialization,
//! and a read-only allowlist for anything invoked outside an explicit user action.

mod env;
mod log;
mod pool;
mod version;

pub use env::pinned_env;
pub use log::{LogEntry, OperationLog};
pub use pool::{Intent, ProcessLayer};
pub use version::{check_git_version, GitVersion, MIN_GIT_VERSION};

use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct GitResult {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub duration: Duration,
}

impl GitResult {
    pub fn ok(&self) -> bool {
        self.status == 0
    }

    pub fn stdout_utf8_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("git exited {status}: {stderr}")]
    NonZeroExit { status: i32, stderr: String },
    #[error("git command timed out after {0:?}")]
    Timeout(Duration),
    #[error("git command was cancelled")]
    Cancelled,
    #[error("io error spawning git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("refused: {0} is not on the read-only allowlist and no write intent was given")]
    NotAllowlisted(String),
}

#[derive(Debug, Clone)]
pub struct GitCall {
    pub repo_root: PathBuf,
    pub args: Vec<String>,
    pub stdin: Option<Vec<u8>>,
    pub extra_env: Vec<(String, String)>,
}

impl GitCall {
    pub fn new(
        repo_root: impl Into<PathBuf>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            args: args.into_iter().map(Into::into).collect(),
            stdin: None,
            extra_env: Vec::new(),
        }
    }

    pub fn with_stdin(mut self, input: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(input.into());
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.extra_env.push((key.into(), val.into()));
        self
    }
}
