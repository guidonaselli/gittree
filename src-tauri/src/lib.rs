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
    diff_file_with_options, diff_revisions, discard_tracked_paths, head_is_published, query_blame,
    query_commit_detail, query_file_at_revision, query_history_count, query_history_graph,
    query_history_page, query_repository_state, query_submodule_matrix,
    query_working_copy_status_cancellable, read_blob_base64, read_working_tree_file_base64,
    stage_hunks, stage_lines,
    stage_paths, stash_paths, unstage_hunks, unstage_lines, unstage_paths, BlameLine, BlameOptions,
    CommitDetail, CommitMessageTemplate, CommitOptions, CommitSummary,
    DiffViewOptions, FileDiff, GraphResult, HistoricalFile, HistoryScope, HistorySearchOptions,
    HistorySearchResult, IgnoreExplanation, IgnoreTarget, OpenOutcome, RepositoryState, Resolved,
    SubmoduleState, WorkingCopyStatus,
};
use repo_state::{
    add_ignore_rule, check_ignore, checkout_branch, compare_branches, create_branch,
    create_tracking_branch, delete_branch, query_branches, rename_branch, stash_and_checkout,
    BranchComparison, BranchEntry, CheckoutOutcome, DeleteBranchOutcome,
};
use repo_state::{
    apply_stash, clear_stashes, create_stash, create_tag, delete_remote_tag, delete_tag,
    drop_stash, inspect_stash, pop_stash, push_tag, query_remote_tags, query_stashes, query_tags,
    CreateStashOptions, CreateTagOptions, PushTagOptions, StashApplyOutcome, StashDetail,
    StashEntry, TagEntry,
};
use serde::Serialize;
use settings::{Bookmark, BookmarksState, Settings, SettingsLoadResult};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use tokio_util::sync::CancellationToken;
use watcher::WatcherState;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct AppState {
    pub process_layer: Arc<ProcessLayer>,
}

#[derive(Default)]
pub struct SearchState(pub Mutex<Option<CancellationToken>>);

#[derive(Default)]
pub struct WorkingCopyScanState(pub Mutex<Option<CancellationToken>>);

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
    scan_state: State<'_, WorkingCopyScanState>,
    root: String,
) -> Result<Resolved<WorkingCopyStatus>, String> {
    let root = PathBuf::from(root);
    let token = CancellationToken::new();
    {
        let mut guard = scan_state.0.lock().map_err(|e| e.to_string())?;
        if let Some(prev) = guard.replace(token.clone()) {
            prev.cancel();
        }
    }
    let res = query_working_copy_status_cancellable(&state.process_layer, &root, token).await;
    Ok(res)
}

#[tauri::command]
fn cancel_working_copy_status(scan_state: State<'_, WorkingCopyScanState>) {
    if let Ok(mut guard) = scan_state.0.lock() {
        if let Some(token) = guard.take() {
            token.cancel();
        }
    }
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
async fn get_file_diff_with_options(
    state: State<'_, AppState>,
    root: String,
    path: String,
    staged: bool,
    context_lines: Option<u32>,
    ignore_whitespace: bool,
) -> Result<Option<FileDiff>, String> {
    let options = DiffViewOptions {
        context_lines,
        ignore_whitespace,
    };
    diff_file_with_options(
        &state.process_layer,
        &PathBuf::from(root),
        &path,
        staged,
        &options,
    )
    .await
}

#[tauri::command]
async fn get_revision_diff(
    state: State<'_, AppState>,
    root: String,
    old_rev: String,
    old_path: Option<String>,
    new_rev: String,
    new_path: Option<String>,
    context_lines: Option<u32>,
    ignore_whitespace: bool,
) -> Result<Vec<FileDiff>, String> {
    let options = DiffViewOptions {
        context_lines,
        ignore_whitespace,
    };
    diff_revisions(
        &state.process_layer,
        &PathBuf::from(root),
        &old_rev,
        old_path.as_deref(),
        &new_rev,
        new_path.as_deref(),
        &options,
    )
    .await
}

#[tauri::command]
async fn get_file_at_revision(
    state: State<'_, AppState>,
    root: String,
    rev: String,
    path: String,
) -> Result<HistoricalFile, String> {
    query_file_at_revision(&state.process_layer, &PathBuf::from(root), &rev, &path).await
}

#[tauri::command]
async fn search_history(
    state: State<'_, AppState>,
    search_state: State<'_, SearchState>,
    root: String,
    options: HistorySearchOptions,
) -> Result<HistorySearchResult, String> {
    let token = CancellationToken::new();
    {
        let mut guard = search_state.0.lock().unwrap();
        if let Some(prev) = guard.take() {
            prev.cancel();
        }
        *guard = Some(token.clone());
    }
    repo_state::search_history(
        &state.process_layer,
        &PathBuf::from(root),
        &options,
        token,
    )
    .await
}

#[tauri::command]
fn cancel_history_search(search_state: State<'_, SearchState>) {
    let mut guard = search_state.0.lock().unwrap();
    if let Some(token) = guard.take() {
        token.cancel();
    }
}

#[tauri::command]
async fn get_blob_base64(
    state: State<'_, AppState>,
    root: String,
    sha: String,
) -> Result<String, String> {
    read_blob_base64(&state.process_layer, &PathBuf::from(root), &sha).await
}

#[tauri::command]
async fn get_working_tree_file_base64(root: String, path: String) -> Result<String, String> {
    read_working_tree_file_base64(&PathBuf::from(root), &path).await
}

#[tauri::command]
async fn get_blame(
    state: State<'_, AppState>,
    root: String,
    path: String,
    ignore_whitespace: bool,
    rev: Option<String>,
) -> Result<Vec<BlameLine>, String> {
    let options = BlameOptions {
        ignore_whitespace,
        rev,
    };
    query_blame(&state.process_layer, &PathBuf::from(root), &path, &options).await
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
async fn check_ignore_path(
    state: State<'_, AppState>,
    root: String,
    path: String,
) -> Result<Option<IgnoreExplanation>, String> {
    let root = PathBuf::from(root);
    check_ignore(&state.process_layer, &root, &path).await
}

#[tauri::command]
async fn add_ignore_rule_command(
    state: State<'_, AppState>,
    root: String,
    target: IgnoreTarget,
    pattern: String,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    add_ignore_rule(&state.process_layer, &root, target, &pattern).await
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

#[tauri::command]
async fn get_branches(
    state: State<'_, AppState>,
    root: String,
) -> Result<Vec<BranchEntry>, String> {
    let root = PathBuf::from(root);
    query_branches(&state.process_layer, &root).await
}

#[tauri::command]
async fn create_branch_command(
    state: State<'_, AppState>,
    root: String,
    name: String,
    start_point: Option<String>,
    checkout: bool,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    create_branch(&state.process_layer, &root, &name, start_point.as_deref(), checkout).await
}

#[tauri::command]
async fn create_tracking_branch_command(
    state: State<'_, AppState>,
    root: String,
    name: String,
    remote_branch: String,
    checkout: bool,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    create_tracking_branch(&state.process_layer, &root, &name, &remote_branch, checkout).await
}

#[tauri::command]
async fn rename_branch_command(
    state: State<'_, AppState>,
    root: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    rename_branch(&state.process_layer, &root, &old_name, &new_name).await
}

#[tauri::command]
async fn delete_branch_command(
    state: State<'_, AppState>,
    root: String,
    name: String,
    force: bool,
) -> Result<DeleteBranchOutcome, String> {
    let root = PathBuf::from(root);
    delete_branch(&state.process_layer, &root, &name, force).await
}

#[tauri::command]
async fn checkout_branch_command(
    state: State<'_, AppState>,
    root: String,
    name: String,
) -> Result<CheckoutOutcome, String> {
    let root = PathBuf::from(root);
    checkout_branch(&state.process_layer, &root, &name).await
}

#[tauri::command]
async fn stash_and_checkout_command(
    state: State<'_, AppState>,
    root: String,
    target: String,
    message: Option<String>,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    stash_and_checkout(&state.process_layer, &root, &target, message.as_deref()).await
}

#[tauri::command]
async fn compare_branches_command(
    state: State<'_, AppState>,
    root: String,
    base: String,
    target: String,
) -> Result<BranchComparison, String> {
    let root = PathBuf::from(root);
    compare_branches(&state.process_layer, &root, &base, &target).await
}

// Tags
#[tauri::command]
async fn get_tags(
    state: State<'_, AppState>,
    root: String,
) -> Result<Vec<TagEntry>, String> {
    let root = PathBuf::from(root);
    query_tags(&state.process_layer, &root).await
}

#[tauri::command]
async fn create_tag_command(
    state: State<'_, AppState>,
    root: String,
    opts: CreateTagOptions,
) -> Result<TagEntry, String> {
    let root = PathBuf::from(root);
    create_tag(&state.process_layer, &root, opts).await
}

#[tauri::command]
async fn delete_tag_command(
    state: State<'_, AppState>,
    root: String,
    name: String,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    delete_tag(&state.process_layer, &root, &name).await
}

#[tauri::command]
async fn push_tag_command(
    state: State<'_, AppState>,
    root: String,
    opts: PushTagOptions,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    push_tag(&state.process_layer, &root, opts).await
}

#[tauri::command]
async fn delete_remote_tag_command(
    state: State<'_, AppState>,
    root: String,
    remote: String,
    name: String,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    delete_remote_tag(&state.process_layer, &root, &remote, &name).await
}

#[tauri::command]
async fn get_remote_tags(
    state: State<'_, AppState>,
    root: String,
    remote: String,
) -> Result<Vec<String>, String> {
    let root = PathBuf::from(root);
    query_remote_tags(&state.process_layer, &root, &remote).await
}

// Stashes
#[tauri::command]
async fn get_stashes(
    state: State<'_, AppState>,
    root: String,
) -> Result<Vec<StashEntry>, String> {
    let root = PathBuf::from(root);
    query_stashes(&state.process_layer, &root).await
}

#[tauri::command]
async fn inspect_stash_command(
    state: State<'_, AppState>,
    root: String,
    selector: String,
) -> Result<StashDetail, String> {
    let root = PathBuf::from(root);
    inspect_stash(&state.process_layer, &root, &selector).await
}

#[tauri::command]
async fn create_stash_command(
    state: State<'_, AppState>,
    root: String,
    opts: CreateStashOptions,
) -> Result<String, String> {
    let root = PathBuf::from(root);
    create_stash(&state.process_layer, &root, opts).await
}

#[tauri::command]
async fn apply_stash_command(
    state: State<'_, AppState>,
    root: String,
    selector: String,
    reinstate_index: bool,
) -> Result<StashApplyOutcome, String> {
    let root = PathBuf::from(root);
    apply_stash(&state.process_layer, &root, &selector, reinstate_index).await
}

#[tauri::command]
async fn pop_stash_command(
    state: State<'_, AppState>,
    root: String,
    selector: String,
    reinstate_index: bool,
) -> Result<StashApplyOutcome, String> {
    let root = PathBuf::from(root);
    pop_stash(&state.process_layer, &root, &selector, reinstate_index).await
}

#[tauri::command]
async fn drop_stash_command(
    state: State<'_, AppState>,
    root: String,
    selector: String,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    drop_stash(&state.process_layer, &root, &selector).await
}

#[tauri::command]
async fn clear_stashes_command(
    state: State<'_, AppState>,
    root: String,
) -> Result<(), String> {
    let root = PathBuf::from(root);
    clear_stashes(&state.process_layer, &root).await
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
        .manage(SearchState::default())
        .manage(WorkingCopyScanState::default())
        .invoke_handler(tauri::generate_handler![
            get_repository_state,
            get_submodule_matrix,
            get_working_copy_status,
            cancel_working_copy_status,
            stage_working_copy_paths,
            unstage_working_copy_paths,
            get_file_diff,
            get_file_diff_with_options,
            get_revision_diff,
            get_file_at_revision,
            search_history,
            cancel_history_search,
            get_blob_base64,
            get_working_tree_file_base64,
            get_blame,
            stage_file_hunks,
            unstage_file_hunks,
            stage_file_lines,
            unstage_file_lines,
            discard_working_copy_paths,
            delete_untracked_working_copy_paths,
            check_ignore_path,
            add_ignore_rule_command,
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
            get_branches,
            create_branch_command,
            create_tracking_branch_command,
            rename_branch_command,
            delete_branch_command,
            checkout_branch_command,
            stash_and_checkout_command,
            compare_branches_command,
            get_tags,
            create_tag_command,
            delete_tag_command,
            push_tag_command,
            delete_remote_tag_command,
            get_remote_tags,
            get_stashes,
            inspect_stash_command,
            create_stash_command,
            apply_stash_command,
            pop_stash_command,
            drop_stash_command,
            clear_stashes_command,
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
