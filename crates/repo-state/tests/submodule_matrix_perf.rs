//! Generates a synthetic superproject and asserts the local submodule
//! matrix query stays within budget. No large fixture is committed; this
//! test builds its own on every run.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use git_process::ProcessLayer;
use repo_state::query_submodule_matrix;

const SUBMODULE_COUNT: usize = 25;
const LOCAL_BUDGET: Duration = Duration::from_millis(1500);

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

fn init_commit_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "fixture@example.com"]);
    git(dir, &["config", "user.name", "Fixture"]);
    std::fs::write(dir.join("f.txt"), "seed").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "seed"]);
}

/// Builds `count` tiny local repositories, wires them as submodules of one
/// superproject, and advances roughly a third of them past their recorded
/// gitlink — the shape the reference superproject was measured in.
fn generate_superproject(root: &Path, count: usize) {
    init_commit_repo(root);
    for i in 0..count {
        let name = format!("sub-{i:02}");
        let child_dir = root.join(format!("../sources/{name}"));
        init_commit_repo(&child_dir);
        git(
            root,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_dir.to_str().unwrap(),
                &name,
            ],
        );
        if i % 3 == 0 {
            let sub_path = root.join(&name);
            std::fs::write(sub_path.join("g.txt"), "advance").unwrap();
            git(&sub_path, &["add", "."]);
            git(&sub_path, &["commit", "-q", "-m", "advance past gitlink"]);
        }
    }
    git(root, &["commit", "-q", "-m", "wire submodules"]);
}

#[test]
#[ignore = "generates 25 real git repositories; run explicitly (cargo test --workspace -- --ignored) or in CI"]
fn local_matrix_query_meets_budget_on_25_submodules() {
    let root_holder = tempfile::tempdir().unwrap();
    let root = root_holder.path().join("superproject");
    generate_superproject(&root, SUBMODULE_COUNT);

    let layer = ProcessLayer::new(8, Duration::from_secs(30));
    let start = Instant::now();
    let matrix = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(query_submodule_matrix(&layer, &root));
    let elapsed = start.elapsed();

    assert_eq!(matrix.len(), SUBMODULE_COUNT);
    assert!(
        elapsed < LOCAL_BUDGET,
        "local submodule matrix query took {elapsed:?}, budget is {LOCAL_BUDGET:?}"
    );

    let unresolved: Vec<&str> = matrix
        .iter()
        .filter(|s| !s.branch.is_known() || !s.gitlink_divergence.is_known() || !s.dirty.is_known())
        .map(|s| s.name.as_str())
        .collect();
    assert!(
        unresolved.is_empty(),
        "submodules with an unresolved core field: {unresolved:?}"
    );
}

const NETWORK_REFRESH_BUDGET: Duration = Duration::from_millis(10000);

#[test]
#[ignore = "generates 25 real git repositories; run explicitly (cargo test --workspace -- --ignored) or in CI"]
fn network_refresh_and_bulk_operations_meet_budget_on_25_submodules() {
    use repo_state::{
        execute_bulk_reset_to_gitlink, preview_bulk_checkout, preview_bulk_reset_to_gitlink,
        refresh_submodules_network, BulkCheckoutAction, BulkResetAction, SubmoduleRefreshOptions,
    };
    use std::path::PathBuf;
    use tokio_util::sync::CancellationToken;

    let root_holder = tempfile::tempdir().unwrap();
    let root = root_holder.path().join("superproject");
    generate_superproject(&root, SUBMODULE_COUNT);

    // Dirty sub-05 to assert dirty protection across the 25 submodules
    let dirty_sub = root.join("sub-05");
    std::fs::write(dirty_sub.join("uncommitted.txt"), "dirty worktree").unwrap();

    let layer = ProcessLayer::new(8, Duration::from_secs(30));
    let rt = tokio::runtime::Runtime::new().unwrap();

    // 1. Parallel network refresh across 25 submodules with concurrency 8
    let start = Instant::now();
    let refresh_result = rt.block_on(refresh_submodules_network(
        &layer,
        &root,
        SubmoduleRefreshOptions {
            concurrency: Some(8),
            paths: None,
            prune: Some(false),
            tags: Some(false),
        },
        None,
        CancellationToken::new(),
    ));
    let refresh_elapsed = start.elapsed();

    assert!(
        refresh_elapsed < NETWORK_REFRESH_BUDGET,
        "network refresh of 25 submodules took {refresh_elapsed:?}, budget is {NETWORK_REFRESH_BUDGET:?}"
    );
    assert_eq!(refresh_result.total, SUBMODULE_COUNT);
    assert_eq!(refresh_result.succeeded, SUBMODULE_COUNT);
    assert_eq!(refresh_result.failed, 0);

    // 2. Bulk checkout preview with dirty protection
    let paths = Some(vec![
        PathBuf::from("sub-01"),
        PathBuf::from("sub-02"),
        PathBuf::from("sub-05"),
    ]);
    let checkout_preview = rt.block_on(preview_bulk_checkout(&layer, &root, "main", paths));
    assert_eq!(checkout_preview.items.len(), 3);
    let sub05_checkout = checkout_preview
        .items
        .iter()
        .find(|item| item.path.ends_with("sub-05"))
        .expect("sub-05 must be in preview");
    assert!(
        matches!(
            sub05_checkout.action,
            BulkCheckoutAction::SkippedDirty { .. }
        ),
        "sub-05 must be skipped as dirty, got {:?}",
        sub05_checkout.action
    );

    // 3. Bulk reset preview and execute refusing dirty submodule
    let reset_paths = Some(vec![PathBuf::from("sub-00"), PathBuf::from("sub-05")]);
    let reset_preview = rt.block_on(preview_bulk_reset_to_gitlink(
        &layer,
        &root,
        reset_paths.clone(),
    ));
    assert_eq!(reset_preview.items.len(), 2);
    let sub05_reset = reset_preview
        .items
        .iter()
        .find(|item| item.path.ends_with("sub-05"))
        .expect("sub-05 in reset preview");
    assert!(
        matches!(sub05_reset.action, BulkResetAction::SkippedDirty { .. }),
        "sub-05 must be skipped as dirty in reset preview"
    );

    let reset_exec = rt.block_on(execute_bulk_reset_to_gitlink(
        &layer,
        &root,
        reset_paths,
        CancellationToken::new(),
    ));
    assert_eq!(reset_exec.total, 2);
    assert_eq!(reset_exec.succeeded, 1);
    assert_eq!(reset_exec.skipped, 1);
    assert_eq!(
        std::fs::read_to_string(dirty_sub.join("uncommitted.txt")).unwrap(),
        "dirty worktree",
        "dirty file must remain untouched"
    );
}
