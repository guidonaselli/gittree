//! Filesystem watching. Emits a coalesced `repo:changed` event per burst;
//! falls back to polling, with `repo:watch-degraded` naming why, when the
//! native watch cannot be established.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use tauri::{AppHandle, Emitter, Runtime};

const DEBOUNCE_WINDOW: Duration = Duration::from_millis(400);
const POLL_INTERVAL: Duration = Duration::from_secs(3);

fn is_significant_git_path(relative: &Path) -> bool {
    let s = relative.to_string_lossy();
    if matches!(
        s.as_ref(),
        "HEAD" | "MERGE_HEAD" | "CHERRY_PICK_HEAD" | "REVERT_HEAD" | "BISECT_LOG" | "packed-refs"
    ) {
        return true;
    }
    s.starts_with("refs/")
        || s.starts_with("rebase-merge")
        || s.starts_with("rebase-apply")
        || s.starts_with("logs/")
}

/// Only an allowlisted set of git-internal paths counts as a real change; everything else under `.git` is refresh noise.
fn is_significant_change(path: &Path, git_dir: &Path) -> bool {
    match path.strip_prefix(git_dir) {
        Ok(relative) if relative.as_os_str().is_empty() => false,
        Ok(relative) => is_significant_git_path(relative),
        Err(_) => true,
    }
}

type MtimeSignature = Vec<(PathBuf, Option<SystemTime>)>;

fn mtime_signature(paths: &[PathBuf]) -> MtimeSignature {
    let mut sig: Vec<_> = paths
        .iter()
        .map(|p| {
            (
                p.clone(),
                std::fs::metadata(p).ok().and_then(|m| m.modified().ok()),
            )
        })
        .collect();
    sig.sort();
    sig
}

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
    let git_dir = root.join(".git");
    let app_for_native = app.clone();
    let last_signature: Mutex<Option<MtimeSignature>> = Mutex::new(None);
    let debouncer = new_debouncer(DEBOUNCE_WINDOW, move |result: DebounceEventResult| {
        if let Ok(events) = result {
            let significant_paths: Vec<PathBuf> = events
                .iter()
                .filter(|e| {
                    e.kind == notify_debouncer_mini::DebouncedEventKind::Any
                        && is_significant_change(&e.path, &git_dir)
                })
                .map(|e| e.path.clone())
                .collect();
            if significant_paths.is_empty() {
                return;
            }
            let signature = mtime_signature(&significant_paths);
            let mut last = last_signature
                .lock()
                .expect("watcher signature mutex poisoned");
            if last.as_ref() == Some(&signature) {
                return;
            }
            *last = Some(signature);
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

/// Cheap change signature for the polling fallback: HEAD's mtime only.
fn directory_signature(git_dir: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(git_dir.join("HEAD"))
        .ok()
        .and_then(|m| m.modified().ok())
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

    #[test]
    fn index_refresh_and_the_bare_git_dir_are_noise() {
        let git_dir = Path::new("/repo/.git");
        assert!(!is_significant_change(
            Path::new("/repo/.git/index"),
            git_dir
        ));
        assert!(!is_significant_change(
            Path::new("/repo/.git/index.lock"),
            git_dir
        ));
        assert!(!is_significant_change(git_dir, git_dir));
        assert!(!is_significant_change(
            Path::new("/repo/.git/objects/ab"),
            git_dir
        ));
    }

    #[test]
    fn head_refs_and_working_tree_paths_are_significant() {
        let git_dir = Path::new("/repo/.git");
        assert!(is_significant_change(Path::new("/repo/.git/HEAD"), git_dir));
        assert!(is_significant_change(
            Path::new("/repo/.git/refs/heads/main"),
            git_dir
        ));
        assert!(is_significant_change(
            Path::new("/repo/.git/logs/HEAD"),
            git_dir
        ));
        assert!(is_significant_change(Path::new("/repo/f.txt"), git_dir));
    }

    #[test]
    fn mtime_signature_is_stable_for_unchanged_files_and_differs_once_touched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, "a").unwrap();
        let paths = vec![path.clone()];

        let first = mtime_signature(&paths);
        let second = mtime_signature(&paths);
        assert_eq!(first, second);

        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&path, "b").unwrap();
        let third = mtime_signature(&paths);
        assert_ne!(first, third);
    }
}
