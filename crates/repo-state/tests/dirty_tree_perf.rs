//! Generates a synthetic 10 000-file dirty tree and asserts status query
//! meets the performance budget and supports cancellation.
//! Run explicitly with: cargo test --test dirty_tree_perf -- --ignored

use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use git_process::ProcessLayer;
use repo_state::fixtures::generate_dirty_tree;
use repo_state::{query_working_copy_status, query_working_copy_status_cancellable, ChangeCode};

const STATUS_BUDGET: Duration = Duration::from_millis(500);

#[tokio::test]
#[ignore = "generates a 10k-file repository; run explicitly with -- --ignored"]
async fn dirty_tree_status_query_meets_budget_on_10k_files() {
    let td = TempDir::new().unwrap();
    let root = td.path().join("dirty_repo");
    generate_dirty_tree(&root, 10_000).unwrap();

    let layer = ProcessLayer::new(4, Duration::from_secs(10));
    let start = Instant::now();
    let status = query_working_copy_status(&layer, &root)
        .await
        .into_known()
        .expect("status should be known");
    let elapsed = start.elapsed();

    assert!(
        elapsed < STATUS_BUDGET,
        "query_working_copy_status took {elapsed:?}, budget is {STATUS_BUDGET:?}"
    );

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

    assert_eq!(staged_count, 5000);
    assert_eq!(unstaged_count, 2500);
    assert_eq!(status.untracked.len(), 2500);
}

#[tokio::test]
#[ignore = "generates a 10k-file repository; run explicitly with -- --ignored"]
async fn dirty_tree_status_query_supports_cancellation() {
    let td = TempDir::new().unwrap();
    let root = td.path().join("dirty_repo");
    generate_dirty_tree(&root, 10_000).unwrap();

    let layer = ProcessLayer::new(4, Duration::from_secs(10));
    let cancel = CancellationToken::new();
    cancel.cancel();

    let status = query_working_copy_status_cancellable(&layer, &root, cancel).await;
    assert_eq!(status.as_known(), None);
}
