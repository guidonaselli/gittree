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
    pub stderr: String,
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

fn sanitize_arg(arg: &str) -> String {
    if let Some(proto_pos) = arg.find("://") {
        let after_proto = &arg[proto_pos + 3..];
        if let Some(at_pos) = after_proto.find('@') {
            let user_info = &after_proto[..at_pos];
            if let Some(colon_pos) = user_info.find(':') {
                let user = &user_info[..colon_pos];
                let host_and_rest = &after_proto[at_pos..];
                let prefix = &arg[..proto_pos + 3];
                return format!("{prefix}{user}:***{host_and_rest}");
            }
        }
    }
    if arg.to_ascii_lowercase().contains("bearer ") {
        let parts: Vec<&str> = arg.splitn(2, "earer ").collect();
        if parts.len() == 2 {
            return format!("{}earer ***", parts[0]);
        }
    }
    arg.to_string()
}

impl OperationLog {
    pub fn record(
        &self,
        repo_root: PathBuf,
        args: Vec<String>,
        exit_status: i32,
        duration: Duration,
        write: bool,
        stderr: String,
    ) {
        let sanitized_args = args.into_iter().map(|a| sanitize_arg(&a)).collect();
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
            args: sanitized_args,
            exit_status,
            duration_ms: duration.as_millis(),
            timestamp_unix_ms,
            write,
            stderr,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_sanitized_in_log() {
        let log = OperationLog::default();
        log.record(
            PathBuf::from("/test/repo"),
            vec![
                "push".into(),
                "https://user:super_secret_token@github.com/org/repo.git".into(),
                "main".into(),
                "-c".into(),
                "http.extraHeader=Authorization: Bearer my-secret-jwt".into(),
            ],
            0,
            Duration::from_millis(42),
            true,
            String::new(),
        );

        let entries = log.snapshot();
        assert_eq!(entries.len(), 1);
        let args = &entries[0].args;
        assert_eq!(args[0], "push");
        assert_eq!(args[1], "https://user:***@github.com/org/repo.git");
        assert_eq!(args[2], "main");
        assert_eq!(args[3], "-c");
        assert_eq!(args[4], "http.extraHeader=Authorization: Bearer ***");
        assert!(!format!("{:?}", entries).contains("super_secret_token"));
        assert!(!format!("{:?}", entries).contains("my-secret-jwt"));
    }
}
