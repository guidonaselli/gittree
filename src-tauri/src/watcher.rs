//! Filesystem watching. Emits a coalesced `repo:changed` event per burst;
//! falls back to polling, with `repo:watch-degraded` naming why, when the
//! native watch cannot be established.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use tauri::{AppHandle, Emitter};

const DEBOUNCE_WINDOW: Duration = Duration::from_millis(400);
const POLL_INTERVAL: Duration = Duration::from_secs(3);

pub enum WatchHandle {
    // Held only for its Drop impl: dropping the debouncer stops the
    // underlying inotify watch. Never read otherwise.
    #[allow(dead_code)]
    Native(notify_debouncer_mini::Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>),
    Polling(tokio::task::JoinHandle<()>),
}

#[derive(Default)]
pub struct WatcherState(pub Mutex<HashMap<PathBuf, WatchHandle>>);

/// Starts watching `root`. Idempotent per root: calling it again for the
/// same repository replaces the previous watch rather than stacking one.
pub fn start_watching(app: AppHandle, state: &WatcherState, root: PathBuf) {
    stop_watching(state, &root);

    let emit_root = root.clone();
    let app_for_native = app.clone();
    let debouncer = new_debouncer(DEBOUNCE_WINDOW, move |result: DebounceEventResult| {
        if result.is_ok() {
            let _ = app_for_native.emit("repo:changed", emit_root.to_string_lossy().to_string());
        }
    });

    let handle = match debouncer {
        Ok(mut debouncer) => {
            match debouncer.watcher().watch(
                &root,
                notify_debouncer_mini::notify::RecursiveMode::Recursive,
            ) {
                Ok(()) => Some(WatchHandle::Native(debouncer)),
                Err(err) => {
                    tracing::warn!(root = %root.display(), %err, "native watch failed; degrading to polling");
                    None
                }
            }
        }
        Err(err) => {
            tracing::warn!(%err, "failed to create filesystem watcher; degrading to polling");
            None
        }
    };

    let handle = handle.unwrap_or_else(|| {
        let _ = app.emit(
            "repo:watch-degraded",
            serde_json::json!({ "root": root.to_string_lossy(), "reason": "native filesystem watch unavailable (platform watch limit or unsupported path); polling every 3s instead" }),
        );
        spawn_poller(app.clone(), root.clone())
    });

    state
        .0
        .lock()
        .expect("watcher state mutex poisoned")
        .insert(root, handle);
}

pub fn stop_watching(state: &WatcherState, root: &Path) {
    if let Some(WatchHandle::Polling(task)) = state
        .0
        .lock()
        .expect("watcher state mutex poisoned")
        .remove(root)
    {
        task.abort();
    }
    // WatchHandle::Native drops the debouncer here too, which stops the watch.
}

fn spawn_poller(app: AppHandle, root: PathBuf) -> WatchHandle {
    let task = tokio::spawn(async move {
        let git_dir = root.join(".git");
        let mut last_signature = directory_signature(&git_dir);
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let signature = directory_signature(&git_dir);
            if signature != last_signature {
                last_signature = signature;
                let _ = app.emit("repo:changed", root.to_string_lossy().to_string());
            }
        }
    });
    WatchHandle::Polling(task)
}

/// Cheap change signature for the polling fallback: mtime of HEAD and the index.
fn directory_signature(git_dir: &Path) -> Option<std::time::SystemTime> {
    let head = std::fs::metadata(git_dir.join("HEAD"))
        .ok()
        .and_then(|m| m.modified().ok());
    let index = std::fs::metadata(git_dir.join("index"))
        .ok()
        .and_then(|m| m.modified().ok());
    head.max(index)
}
