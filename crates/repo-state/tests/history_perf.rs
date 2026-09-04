//! Generates a synthetic 100k-commit repository via `git fast-import` and
//! asserts a deep page fetch stays within budget. No large fixture is
//! committed; this test builds its own on every run.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use git_process::ProcessLayer;
use repo_state::{query_history_page, HistoryScope};

const COMMIT_COUNT: usize = 100_000;
const PAGE_BUDGET: Duration = Duration::from_secs(2);

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// Builds `count` linear commits on `main` via `fast-import`, which is
/// orders of magnitude faster than `count` discrete `git commit` calls.
fn generate_fixture(dir: &Path, count: usize) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);

    let mut child = Command::new("git")
        .args(["fast-import", "--quiet"])
        .current_dir(dir)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "blob\nmark :1\ndata 5\nhello").unwrap();
        for i in 0..count {
            let mark = i + 2;
            writeln!(stdin, "commit refs/heads/main").unwrap();
            writeln!(stdin, "mark :{mark}").unwrap();
            writeln!(
                stdin,
                "author Fixture <fixture@example.com> {} +0000",
                1_700_000_000 + i as u64
            )
            .unwrap();
            writeln!(
                stdin,
                "committer Fixture <fixture@example.com> {} +0000",
                1_700_000_000 + i as u64
            )
            .unwrap();
            let msg = format!("commit {i}");
            writeln!(stdin, "data {}\n{msg}", msg.len()).unwrap();
            if i > 0 {
                writeln!(stdin, "from :{}", mark - 1).unwrap();
            }
            writeln!(stdin, "M 100644 :1 f{}.txt", i % 20).unwrap();
        }
    }
    let status = child.wait().unwrap();
    assert!(status.success(), "git fast-import failed");
}

#[test]
#[ignore = "generates a 100k-commit repository; run explicitly (cargo test --workspace -- --ignored) or in CI"]
fn a_deep_page_fetch_meets_budget_on_a_100k_commit_history() {
    let root_holder = tempfile::tempdir().unwrap();
    let root = root_holder.path().join("fixture");
    generate_fixture(&root, COMMIT_COUNT);

    let layer = ProcessLayer::new(8, Duration::from_secs(30));
    let start = Instant::now();
    let page = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(query_history_page(
            &layer,
            &root,
            &HistoryScope::CurrentBranch,
            60_000,
            50,
        ))
        .unwrap();
    let elapsed = start.elapsed();

    assert_eq!(
        page.len(),
        50,
        "a deep page must still return a full page of results"
    );
    assert!(
        elapsed < PAGE_BUDGET,
        "a deep page fetch took {elapsed:?}, budget is {PAGE_BUDGET:?}"
    );
}
