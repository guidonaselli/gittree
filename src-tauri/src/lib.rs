mod desktop_theme;
mod settings;
mod watcher;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use desktop_theme::DesktopPalette;
use git_process::{check_git_version, ProcessLayer};
use repo_state::{
    amend, commit, commit_message_template, delete_untracked_paths, diff_file,
    discard_tracked_paths, head_is_published, query_commit_detail, query_history_count,
    query_history_graph, query_history_page, query_repository_state, query_submodule_matrix,
    query_working_copy_status, stage_hunks, stage_lines, stage_paths, stash_paths, unstage_hunks,
    unstage_lines, unstage_paths, CommitDetail, CommitMessageTemplate, CommitOptions,
    CommitSummary, FileDiff, GraphResult, HistoryScope, OpenOutcome, RepositoryState, Resolved,
    SubmoduleState, WorkingCopyStatus,
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

#[tauri::command]
async fn get_working_copy_status(
    state: State<'_, AppState>,
    root: String,
) -> Result<Resolved<WorkingCopyStatus>, String> {
    let root = PathBuf::from(root);
    Ok(query_working_copy_status(&state.process_layer, &root).await)
}

#[tauri::command]
async fn stage_working_copy_paths(
    state: State<'_, AppState>,
    root: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    stage_paths(&state.process_layer, &PathBuf::from(root), &paths).await
}

#[tauri::command]
async fn unstage_working_copy_paths(
    state: State<'_, AppState>,
    root: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    unstage_paths(&state.process_layer, &PathBuf::from(root), &paths).await
}

#[tauri::command]
async fn get_file_diff(
    state: State<'_, AppState>,
    root: String,
    path: String,
    staged: bool,
) -> Result<Option<FileDiff>, String> {
    diff_file(&state.process_layer, &PathBuf::from(root), &path, staged).await
}

#[tauri::command]
async fn stage_file_hunks(
    state: State<'_, AppState>,
    root: String,
    path: String,
    hunk_indices: Vec<usize>,
) -> Result<(), String> {
    stage_hunks(
        &state.process_layer,
        &PathBuf::from(root),
        &path,
        &hunk_indices,
    )
    .await
}

#[tauri::command]
async fn unstage_file_hunks(
    state: State<'_, AppState>,
    root: String,
    path: String,
    hunk_indices: Vec<usize>,
) -> Result<(), String> {
    unstage_hunks(
        &state.process_layer,
        &PathBuf::from(root),
        &path,
        &hunk_indices,
    )
    .await
}

#[tauri::command]
async fn stage_file_lines(
    state: State<'_, AppState>,
    root: String,
    path: String,
    hunk_index: usize,
    line_indices: Vec<usize>,
) -> Result<(), String> {
    stage_lines(
        &state.process_layer,
        &PathBuf::from(root),
        &path,
        hunk_index,
        &line_indices,
    )
    .await
}

#[tauri::command]
async fn unstage_file_lines(
    state: State<'_, AppState>,
    root: String,
    path: String,
    hunk_index: usize,
    line_indices: Vec<usize>,
) -> Result<(), String> {
    unstage_lines(
        &state.process_layer,
        &PathBuf::from(root),
        &path,
        hunk_index,
        &line_indices,
    )
    .await
}

#[tauri::command]
async fn discard_working_copy_paths(
    state: State<'_, AppState>,
    root: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    discard_tracked_paths(&state.process_layer, &PathBuf::from(root), &paths).await
}

#[tauri::command]
async fn delete_untracked_working_copy_paths(
    state: State<'_, AppState>,
    root: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    delete_untracked_paths(&state.process_layer, &PathBuf::from(root), &paths).await
}

#[tauri::command]
async fn stash_working_copy_paths(
    state: State<'_, AppState>,
    root: String,
    paths: Vec<String>,
    message: Option<String>,
) -> Result<(), String> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    stash_paths(
        &state.process_layer,
        &PathBuf::from(root),
        &paths,
        message.as_deref(),
    )
    .await
}

#[tauri::command]
async fn commit_working_copy(
    state: State<'_, AppState>,
    root: String,
    message: String,
    author: Option<String>,
    sign_off: bool,
    sign: bool,
) -> Result<String, String> {
    let options = CommitOptions {
        author,
        sign_off,
        sign,
    };
    commit(
        &state.process_layer,
        &PathBuf::from(root),
        &message,
        &options,
    )
    .await
}

#[tauri::command]
async fn is_head_published(state: State<'_, AppState>, root: String) -> Result<bool, String> {
    head_is_published(&state.process_layer, &PathBuf::from(root)).await
}

#[tauri::command]
async fn get_commit_message_template(
    state: State<'_, AppState>,
    root: String,
) -> Result<Option<CommitMessageTemplate>, String> {
    commit_message_template(&state.process_layer, &PathBuf::from(root)).await
}

#[tauri::command]
async fn get_history_page(
    state: State<'_, AppState>,
    root: String,
    scope: HistoryScope,
    skip: usize,
    limit: usize,
) -> Result<Vec<CommitSummary>, String> {
    query_history_page(
        &state.process_layer,
        &PathBuf::from(root),
        &scope,
        skip,
        limit,
    )
    .await
}

#[tauri::command]
async fn get_history_count(
    state: State<'_, AppState>,
    root: String,
    scope: HistoryScope,
) -> Result<usize, String> {
    query_history_count(&state.process_layer, &PathBuf::from(root), &scope).await
}

#[tauri::command]
async fn get_history_graph(
    state: State<'_, AppState>,
    root: String,
    scope: HistoryScope,
) -> Result<GraphResult, String> {
    query_history_graph(&state.process_layer, &PathBuf::from(root), &scope).await
}

#[tauri::command]
async fn get_commit_detail(
    state: State<'_, AppState>,
    root: String,
    sha: String,
    parent_index: Option<usize>,
) -> Result<CommitDetail, String> {
    query_commit_detail(
        &state.process_layer,
        &PathBuf::from(root),
        &sha,
        parent_index,
    )
    .await
}

#[tauri::command]
async fn amend_working_copy(
    state: State<'_, AppState>,
    root: String,
    message: String,
    author: Option<String>,
    sign_off: bool,
    sign: bool,
) -> Result<String, String> {
    let options = CommitOptions {
        author,
        sign_off,
        sign,
    };
    amend(
        &state.process_layer,
        &PathBuf::from(root),
        &message,
        &options,
    )
    .await
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

#[tauri::command]
fn get_desktop_palette() -> Result<Option<DesktopPalette>, String> {
    desktop_theme::read_published_palette()
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
            get_working_copy_status,
            stage_working_copy_paths,
            unstage_working_copy_paths,
            get_file_diff,
            stage_file_hunks,
            unstage_file_hunks,
            stage_file_lines,
            unstage_file_lines,
            discard_working_copy_paths,
            delete_untracked_working_copy_paths,
            stash_working_copy_paths,
            commit_working_copy,
            is_head_published,
            get_commit_message_template,
            get_history_page,
            get_history_count,
            get_history_graph,
            get_commit_detail,
            amend_working_copy,
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
            get_desktop_palette,
        ])
        .setup(|app| {
            let watch_dir = desktop_theme::published_theme_watch_dir();
            let handle = watcher::start_desktop_theme_watch(app.handle().clone(), watch_dir);
            app.manage(std::sync::Mutex::new(handle));
            Ok(())
        })
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
