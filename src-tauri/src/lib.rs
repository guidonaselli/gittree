mod settings;
mod watcher;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use git_process::{check_git_version, ProcessLayer};
use repo_state::{
    query_repository_state, query_submodule_matrix, OpenOutcome, RepositoryState, SubmoduleState,
};
use serde::Serialize;
use settings::{Bookmark, BookmarksState, Settings, SettingsLoadResult};
use tauri::{AppHandle, Manager, State};
use watcher::WatcherState;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct AppState {
    pub process_layer: Arc<ProcessLayer>,
}

/// Runs before the Tauri builder so a failed version check never produces
/// a half-initialized window.
fn doctor_or_exit() {
    match check_git_version() {
        Ok(version) => {
            tracing::info!(%version, "git version check passed");
        }
        Err(err) => {
            eprintln!("GitTree cannot start: {err}");
            std::process::exit(1);
        }
    }
}

#[tauri::command]
async fn get_repository_state(
    state: State<'_, AppState>,
    root: String,
) -> Result<RepositoryState, String> {
    let root = PathBuf::from(root);
    let git_dir = root.join(".git");
    Ok(query_repository_state(&state.process_layer, &root, &git_dir).await)
}

#[tauri::command]
async fn get_submodule_matrix(
    state: State<'_, AppState>,
    root: String,
) -> Result<Vec<SubmoduleState>, String> {
    let root = PathBuf::from(root);
    Ok(query_submodule_matrix(&state.process_layer, &root).await)
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", content = "root")]
enum OpenRepositoryResult {
    Repository(String),
    BareRepository(String),
    NotARepository,
}

#[tauri::command]
async fn resolve_repository_root(
    state: State<'_, AppState>,
    path: String,
) -> Result<OpenRepositoryResult, String> {
    let outcome =
        repo_state::resolve_open(&state.process_layer, std::path::Path::new(&path)).await?;
    Ok(match outcome {
        OpenOutcome::Repository { root } => {
            OpenRepositoryResult::Repository(root.to_string_lossy().to_string())
        }
        OpenOutcome::BareRepository { root } => {
            OpenRepositoryResult::BareRepository(root.to_string_lossy().to_string())
        }
        OpenOutcome::NotARepository => OpenRepositoryResult::NotARepository,
    })
}

#[tauri::command]
async fn init_repository(state: State<'_, AppState>, path: String) -> Result<String, String> {
    repo_state::init_repository(&state.process_layer, std::path::Path::new(&path)).await?;
    let outcome =
        repo_state::resolve_open(&state.process_layer, std::path::Path::new(&path)).await?;
    match outcome {
        OpenOutcome::Repository { root } => Ok(root.to_string_lossy().to_string()),
        _ => Err("repository was created but could not be resolved".to_string()),
    }
}

#[tauri::command]
fn get_operation_log(state: State<'_, AppState>) -> Vec<git_process::LogEntry> {
    state.process_layer.log.snapshot()
}

#[tauri::command]
fn get_settings() -> SettingsLoadResult {
    settings::load_settings()
}

#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    settings::save_settings(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_bookmarks(state: State<'_, BookmarksState>) -> Vec<Bookmark> {
    state.0.lock().expect("bookmarks mutex poisoned").clone()
}

#[tauri::command]
fn add_bookmark(
    state: State<'_, BookmarksState>,
    root: String,
    group: Option<String>,
) -> Result<Vec<Bookmark>, String> {
    let mut bookmarks = state.0.lock().expect("bookmarks mutex poisoned");
    if !bookmarks.iter().any(|b| b.root == root) {
        let order = bookmarks.len() as i32;
        bookmarks.push(Bookmark { root, group, order });
        settings::save_bookmarks(&bookmarks).map_err(|e| e.to_string())?;
    }
    Ok(bookmarks.clone())
}

#[tauri::command]
fn remove_bookmark(
    state: State<'_, BookmarksState>,
    root: String,
) -> Result<Vec<Bookmark>, String> {
    let mut bookmarks = state.0.lock().expect("bookmarks mutex poisoned");
    bookmarks.retain(|b| b.root != root);
    settings::save_bookmarks(&bookmarks).map_err(|e| e.to_string())?;
    Ok(bookmarks.clone())
}

#[tauri::command]
fn start_watching(app: AppHandle, watcher_state: State<'_, WatcherState>, root: String) {
    watcher::start_watching(app, &watcher_state, PathBuf::from(root));
}

#[tauri::command]
fn stop_watching(watcher_state: State<'_, WatcherState>, root: String) {
    watcher::stop_watching(&watcher_state, &PathBuf::from(root));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    doctor_or_exit();

    let initial_settings = settings::load_settings();
    for warning in &initial_settings.warnings {
        tracing::warn!(%warning, "settings.json: falling back for one key");
    }
    let process_layer = Arc::new(ProcessLayer::new(
        initial_settings.settings.concurrency,
        DEFAULT_TIMEOUT,
    ));

    tauri::Builder::default()
        .manage(AppState { process_layer })
        .manage(BookmarksState::default())
        .manage(WatcherState::default())
        .invoke_handler(tauri::generate_handler![
            get_repository_state,
            get_submodule_matrix,
            resolve_repository_root,
            get_settings,
            save_settings,
            get_bookmarks,
            add_bookmark,
            remove_bookmark,
            start_watching,
            stop_watching,
            get_operation_log,
            init_repository,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Stop every watch rather than leaking on window close.
                let state = window.state::<WatcherState>();
                let mut watches = state.0.lock().expect("watcher state mutex poisoned");
                for (_, handle) in watches.drain() {
                    if let watcher::WatchHandle::Polling(task) = handle {
                        task.abort();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running gittree");
}
