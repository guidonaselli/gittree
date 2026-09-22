use std::time::Duration;
use tempfile::TempDir;

use git_process::ProcessLayer;
use repo_state::fixtures::{
    generate_dirty_tree, generate_large_history, generate_submodule_superproject,
};
use repo_state::{
    query_history_count, query_submodule_matrix, query_working_copy_status, ChangeCode,
    GitlinkDivergence, HistoryScope,
};

#[tokio::test]
async fn test_generate_submodule_superproject() {
    let td = TempDir::new().unwrap();
    let super_root = td.path().join("super");
    generate_submodule_superproject(&super_root, 6).unwrap();

    let layer = ProcessLayer::new(4, Duration::from_secs(5));
    let entries = query_submodule_matrix(&layer, &super_root).await;
    assert_eq!(entries.len(), 6);

    let diverged = entries
        .iter()
        .filter(|e| {
            matches!(
                e.gitlink_divergence.as_known(),
                Some(GitlinkDivergence::Ahead { .. })
                    | Some(GitlinkDivergence::Behind { .. })
                    | Some(GitlinkDivergence::Both { .. })
            )
        })
        .count();
    assert!(diverged >= 2);
}

#[tokio::test]
async fn test_generate_large_history() {
    let td = TempDir::new().unwrap();
    let root = td.path().join("history_repo");
    generate_large_history(&root, 50, 3).unwrap();

    let layer = ProcessLayer::new(4, Duration::from_secs(5));
    let count = query_history_count(&layer, &root, &HistoryScope::AllBranches)
        .await
        .unwrap();
    assert_eq!(count, 50);
}

#[tokio::test]
async fn test_generate_dirty_tree() {
    let td = TempDir::new().unwrap();
    let root = td.path().join("dirty_repo");
    generate_dirty_tree(&root, 40).unwrap();

    let layer = ProcessLayer::new(4, Duration::from_secs(5));
    let status = query_working_copy_status(&layer, &root)
        .await
        .into_known()
        .expect("status should be known");

    // 40 files: 10 staged modifications, 10 staged additions, 10 unstaged modifications, 10 untracked
    let staged_count = status
        .changed
        .iter()
        .filter(|c| c.staged != ChangeCode::Unmodified)
        .count();
    let unstaged_count = status
        .changed
        .iter()
        .filter(|c| c.unstaged != ChangeCode::Unmodified)
        .count();
    assert_eq!(staged_count, 20);
    assert_eq!(unstaged_count, 10);
    assert_eq!(status.untracked.len(), 10);
}
