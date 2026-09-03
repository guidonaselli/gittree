use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use git_process::{check_git_version, ProcessLayer};
use repo_state::{query_repository_state, query_submodule_matrix, RepositoryState, SubmoduleState};
use tauri::State;

const DEFAULT_CONCURRENCY: usize = 8;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct AppState {
    pub process_layer: Arc<ProcessLayer>,
}

/// T-002 / task 1.2: verify the `git` version floor before the app is
/// allowed to start at all. This runs before the Tauri builder so a failure
/// never produces a half-initialized window.
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
async fn resolve_repository_root(path: String) -> Result<String, String> {
    let call = git_process::GitCall::new(&path, ["rev-parse", "--show-toplevel"]);
    let layer = ProcessLayer::new(1, Duration::from_secs(10));
    let result = layer
        .run(
            call,
            git_process::Intent::Read,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!(
            "no repository found at {path}: {}",
            result.stderr.trim()
        ));
    }
    Ok(result.stdout_utf8_lossy().trim().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    doctor_or_exit();

    let process_layer = Arc::new(ProcessLayer::new(DEFAULT_CONCURRENCY, DEFAULT_TIMEOUT));

    tauri::Builder::default()
        .manage(AppState { process_layer })
        .invoke_handler(tauri::generate_handler![
            get_repository_state,
            get_submodule_matrix,
            resolve_repository_root,
        ])
        .run(tauri::generate_context!())
        .expect("error while running gittree");
}
