//! Manual verification against a real repository path, not part of the
//! automated suite (which uses generated fixtures per design D8). Run with:
//! `cargo run -p repo-state --example bench_matrix -- <path>`
use git_process::ProcessLayer;
use repo_state::query_submodule_matrix;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: bench_matrix <superproject-path>");
    let layer = ProcessLayer::new(8, Duration::from_secs(30));
    let start = Instant::now();
    let matrix = query_submodule_matrix(&layer, std::path::Path::new(&path)).await;
    let elapsed = start.elapsed();
    println!(
        "Queried {} submodules in {:?} (budget: 1.5s)",
        matrix.len(),
        elapsed
    );
    let unresolved: usize = matrix
        .iter()
        .filter(|s| !s.branch.is_known() || !s.gitlink_divergence.is_known() || !s.dirty.is_known())
        .count();
    println!("Submodules with any unresolved core field: {unresolved}");
    for s in &matrix {
        println!(
            "{:28} initialized={:5} branch={:?} gitlink={:?} dirty={:?}",
            s.name, s.initialized, s.branch, s.gitlink_divergence, s.dirty
        );
    }
}
