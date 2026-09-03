use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// One executed command, kept for the in-app operation log.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogEntry {
    pub repo_root: PathBuf,
    pub args: Vec<String>,
    pub exit_status: i32,
    pub duration_ms: u128,
    pub timestamp_unix_ms: u128,
    pub write: bool,
}

const MAX_ENTRIES: usize = 2000;

pub struct OperationLog {
    entries: Mutex<VecDeque<LogEntry>>,
}

impl Default for OperationLog {
    fn default() -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(MAX_ENTRIES)),
        }
    }
}

impl OperationLog {
    pub fn record(
        &self,
        repo_root: PathBuf,
        args: Vec<String>,
        exit_status: i32,
        duration: Duration,
        write: bool,
    ) {
        let timestamp_unix_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let mut entries = self.entries.lock().expect("operation log mutex poisoned");
        if entries.len() >= MAX_ENTRIES {
            entries.pop_front();
        }
        entries.push_back(LogEntry {
            repo_root,
            args,
            exit_status,
            duration_ms: duration.as_millis(),
            timestamp_unix_ms,
            write,
        });
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.entries
            .lock()
            .expect("operation log mutex poisoned")
            .iter()
            .cloned()
            .collect()
    }

    /// True if any entry after `from_len` is a write.
    pub fn any_write_since(&self, from_len: usize) -> bool {
        let entries = self.entries.lock().expect("operation log mutex poisoned");
        entries.iter().skip(from_len).any(|e| e.write)
    }

    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .expect("operation log mutex poisoned")
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
