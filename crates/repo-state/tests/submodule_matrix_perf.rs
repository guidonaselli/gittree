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
