//! Filesystem watching. Emits a coalesced `repo:changed` event per burst;
//! falls back to polling, with `repo:watch-degraded` naming why, when the
//! native watch cannot be established.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use tauri::{AppHandle, Emitter, Runtime};

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
pub fn start_watching<R: Runtime>(app: AppHandle<R>, state: &WatcherState, root: PathBuf) {
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

/// Watches the theme file's parent dir (a theme switch may rename the file) and emits `desktop-theme:changed`.
pub fn start_desktop_theme_watch<R: Runtime>(
    app: AppHandle<R>,
    watch_dir: PathBuf,
) -> Option<WatchHandle> {
    let debouncer = new_debouncer(DEBOUNCE_WINDOW, move |result: DebounceEventResult| {
        if result.is_ok() {
            let _ = app.emit("desktop-theme:changed", ());
        }
    });
    let mut debouncer = debouncer.ok()?;
    debouncer
        .watcher()
        .watch(
            &watch_dir,
            notify_debouncer_mini::notify::RecursiveMode::Recursive,
        )
        .ok()?;
    Some(WatchHandle::Native(debouncer))
}

fn spawn_poller<R: Runtime>(app: AppHandle<R>, root: PathBuf) -> WatchHandle {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use tauri::Listener;

    fn init_repo(dir: &Path) {
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(dir)
                .status()
                .unwrap();
            assert!(status.success());
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        std::fs::write(dir.join("f.txt"), "hello").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
    }

    #[test]
    fn external_commit_triggers_a_repo_changed_event() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());

        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        let watcher_state = WatcherState::default();

        let (tx, rx) = mpsc::channel::<String>();
        handle.listen_any("repo:changed", move |event| {
            let root: String = serde_json::from_str(event.payload()).unwrap();
            let _ = tx.send(root);
        });

        start_watching(handle.clone(), &watcher_state, dir.path().to_path_buf());

        // A commit made outside the app, exactly the scenario the watcher exists for.
        std::fs::write(dir.path().join("f.txt"), "changed").unwrap();
        std::process::Command::new("git")
            .args(["commit", "-aqm", "external change"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let received = rx.recv_timeout(Duration::from_secs(3));
        assert!(
            received.is_ok(),
            "expected a repo:changed event after an external commit"
        );

        stop_watching(&watcher_state, dir.path());
    }
}
