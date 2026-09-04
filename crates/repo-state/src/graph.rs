use std::collections::HashMap;
use std::path::Path;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

use crate::history::HistoryScope;

/// The most lanes rendered side by side before overflow collapsing kicks in.
pub const LANE_BUDGET: usize = 12;
const OVERFLOW_LANE: usize = LANE_BUDGET - 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphRow {
    pub sha: String,
    pub parents: Vec<String>,
    pub lane: usize,
    pub parent_lanes: Vec<usize>,
    pub is_merge: bool,
    pub overflow: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphResult {
    pub rows: Vec<GraphRow>,
    pub max_lane: usize,
}

struct RawCommit {
    sha: String,
    parents: Vec<String>,
}

async fn fetch_all_commits(
    layer: &ProcessLayer,
    root: &Path,
    scope: &HistoryScope,
) -> Result<Vec<RawCommit>, String> {
    let mut args = vec!["log".to_string(), "--date-order".to_string()];
    match scope {
        HistoryScope::CurrentBranch | HistoryScope::Path(_) => {}
        HistoryScope::AllBranches => args.push("--branches".to_string()),
        HistoryScope::AllRefs => args.push("--all".to_string()),
    }
    args.push("--format=%H %P".to_string());
    args.push("-z".to_string());
    if let HistoryScope::Path(path) = scope {
        args.push("--".to_string());
        args.push(path.clone());
    }

    let result = layer
        .run(
            GitCall::new(root, args),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(result.stderr.trim().to_string());
    }

    Ok(result
        .stdout_utf8_lossy()
        .split('\0')
        .filter(|record| !record.is_empty())
        .map(|record| {
            let mut parts = record.split(' ');
            let sha = parts.next().unwrap_or("").to_string();
            let parents = parts
                .filter(|p| !p.is_empty())
                .map(|p| p.to_string())
                .collect();
            RawCommit { sha, parents }
        })
        .collect())
}

/// Active-lanes graph layout, collapsing lanes beyond `LANE_BUDGET`.
fn assign_lanes(commits: &[RawCommit]) -> GraphResult {
    let mut active: Vec<Option<String>> = Vec::new();
    let mut lane_of_commit: HashMap<String, usize> = HashMap::new();
    let mut raw_lanes: Vec<usize> = Vec::with_capacity(commits.len());

    let open_lane = |active: &mut Vec<Option<String>>| -> usize {
        match active.iter().position(|s| s.is_none()) {
            Some(l) => l,
            None => {
                active.push(None);
                active.len() - 1
            }
        }
    };

    for commit in commits {
        let lane = lane_of_commit
            .remove(&commit.sha)
            .unwrap_or_else(|| open_lane(&mut active));
        raw_lanes.push(lane);

        if let Some(first_parent) = commit.parents.first() {
            active[lane] = Some(first_parent.clone());
            lane_of_commit.insert(first_parent.clone(), lane);
        } else {
            active[lane] = None;
        }

        for parent in commit.parents.iter().skip(1) {
            if !lane_of_commit.contains_key(parent) {
                let new_lane = open_lane(&mut active);
                active[new_lane] = Some(parent.clone());
                lane_of_commit.insert(parent.clone(), new_lane);
            }
        }
    }

    let max_raw_lane = raw_lanes.iter().copied().max().unwrap_or(0);
    let sha_to_display_lane: HashMap<&str, usize> = commits
        .iter()
        .zip(raw_lanes.iter())
        .map(|(c, &lane)| (c.sha.as_str(), lane.min(OVERFLOW_LANE)))
        .collect();

    let rows = commits
        .iter()
        .zip(raw_lanes.iter())
        .map(|(commit, &raw_lane)| {
            let lane = raw_lane.min(OVERFLOW_LANE);
            let parent_lanes = commit
                .parents
                .iter()
                .filter_map(|p| sha_to_display_lane.get(p.as_str()).copied())
                .collect();
            GraphRow {
                sha: commit.sha.clone(),
                parents: commit.parents.clone(),
                lane,
                parent_lanes,
                is_merge: commit.parents.len() > 1,
                overflow: raw_lane > OVERFLOW_LANE,
            }
        })
        .collect();

    GraphResult {
        rows,
        max_lane: max_raw_lane.min(OVERFLOW_LANE),
    }
}

pub async fn query_history_graph(
    layer: &ProcessLayer,
    root: &Path,
    scope: &HistoryScope,
) -> Result<GraphResult, String> {
    let commits = fetch_all_commits(layer, root, scope).await?;
    Ok(assign_lanes(&commits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        git(dir.path(), &["config", "user.email", "test@example.com"]);
        git(dir.path(), &["config", "user.name", "Test"]);
        dir
    }

    fn commit(dir: &Path, name: &str, message: &str) {
        std::fs::write(dir.join(name), message).unwrap();
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", message]);
    }

    #[tokio::test]
    async fn a_linear_history_stays_on_a_single_lane() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "first");
        commit(dir.path(), "b.txt", "second");
        commit(dir.path(), "c.txt", "third");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let graph = query_history_graph(&layer, dir.path(), &HistoryScope::CurrentBranch)
            .await
            .unwrap();

        assert_eq!(graph.rows.len(), 3);
        assert!(graph.rows.iter().all(|r| r.lane == 0));
        assert!(graph.rows.iter().all(|r| !r.is_merge));
        assert_eq!(graph.max_lane, 0);
    }

    #[tokio::test]
    async fn a_merge_commit_is_flagged_and_reunites_its_branch_lane() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "on main");
        git(dir.path(), &["checkout", "-q", "-b", "feature"]);
        commit(dir.path(), "b.txt", "on feature");
        git(dir.path(), &["checkout", "-q", "main"]);
        commit(dir.path(), "c.txt", "second on main");
        git(
            dir.path(),
            &["merge", "-q", "--no-ff", "-m", "merge feature", "feature"],
        );

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let graph = query_history_graph(&layer, dir.path(), &HistoryScope::CurrentBranch)
            .await
            .unwrap();

        let merge_row = &graph.rows[0];
        assert!(merge_row.is_merge);
        assert_eq!(merge_row.parent_lanes.len(), 2);
        assert!(
            graph.max_lane >= 1,
            "the feature branch must have opened a second lane"
        );
    }

    #[tokio::test]
    async fn lanes_beyond_the_budget_are_marked_overflow_and_collapsed() {
        let dir = init_repo();
        commit(dir.path(), "base.txt", "base");
        git(dir.path(), &["checkout", "-q", "main"]);

        // Open more unmerged sibling branches than the lane budget allows.
        for i in 0..(LANE_BUDGET + 2) {
            git(
                dir.path(),
                &["checkout", "-q", "-b", &format!("branch-{i}"), "main"],
            );
            commit(dir.path(), &format!("f{i}.txt"), &format!("on branch {i}"));
        }

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let graph = query_history_graph(&layer, dir.path(), &HistoryScope::AllBranches)
            .await
            .unwrap();

        assert!(
            graph.rows.iter().any(|r| r.overflow),
            "some row must be marked overflow"
        );
        assert!(
            graph.rows.iter().all(|r| r.lane < LANE_BUDGET),
            "no displayed lane may exceed the budget"
        );
        assert_eq!(graph.max_lane, LANE_BUDGET - 1);
    }

    #[tokio::test]
    async fn lane_assignment_is_deterministic_across_repeated_calls() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "on main");
        git(dir.path(), &["checkout", "-q", "-b", "feature"]);
        commit(dir.path(), "b.txt", "on feature");
        git(dir.path(), &["checkout", "-q", "main"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let first = query_history_graph(&layer, dir.path(), &HistoryScope::AllBranches)
            .await
            .unwrap();
        let second = query_history_graph(&layer, dir.path(), &HistoryScope::AllBranches)
            .await
            .unwrap();

        assert_eq!(
            first, second,
            "lane assignment must be stable across repeated fetches"
        );
    }

    #[tokio::test]
    async fn graph_row_order_matches_the_paginated_history_list_order() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "on main");
        git(dir.path(), &["checkout", "-q", "-b", "feature"]);
        commit(dir.path(), "b.txt", "on feature");
        git(dir.path(), &["checkout", "-q", "main"]);
        commit(dir.path(), "c.txt", "second on main");
        git(
            dir.path(),
            &["merge", "-q", "--no-ff", "-m", "merge feature", "feature"],
        );

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let graph = query_history_graph(&layer, dir.path(), &HistoryScope::CurrentBranch)
            .await
            .unwrap();
        let page = crate::history::query_history_page(
            &layer,
            dir.path(),
            &HistoryScope::CurrentBranch,
            0,
            100,
        )
        .await
        .unwrap();

        let graph_shas: Vec<&str> = graph.rows.iter().map(|r| r.sha.as_str()).collect();
        let page_shas: Vec<&str> = page.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(
            graph_shas, page_shas,
            "the graph and the paginated list must agree on row order so lane indices line up by position"
        );
    }
}
