use std::path::Path;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "kind", content = "path")]
pub enum HistoryScope {
    CurrentBranch,
    AllBranches,
    AllRefs,
    Path(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CommitSummary {
    pub sha: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub subject: String,
}

const FIELD_SEP: char = '\u{1f}';

fn parse_record(record: &str) -> Option<CommitSummary> {
    if record.is_empty() {
        return None;
    }
    let mut fields = record.split(FIELD_SEP);
    let sha = fields.next()?.to_string();
    let parents = fields
        .next()?
        .split(' ')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect();
    let author_name = fields.next()?.to_string();
    let author_email = fields.next()?.to_string();
    let author_date = fields.next()?.to_string();
    let subject = fields.next().unwrap_or("").to_string();
    Some(CommitSummary {
        sha,
        parents,
        author_name,
        author_email,
        author_date,
        subject,
    })
}

/// Fetches one page of history. Only `limit` commits are ever held in
/// memory at once, regardless of total repository size.
pub async fn query_history_page(
    layer: &ProcessLayer,
    root: &Path,
    scope: &HistoryScope,
    skip: usize,
    limit: usize,
) -> Result<Vec<CommitSummary>, String> {
    let mut args = vec!["log".to_string(), "--date-order".to_string()];
    match scope {
        HistoryScope::CurrentBranch | HistoryScope::Path(_) => {}
        HistoryScope::AllBranches => args.push("--branches".to_string()),
        HistoryScope::AllRefs => args.push("--all".to_string()),
    }
    args.push(format!("--skip={skip}"));
    args.push(format!("--max-count={limit}"));
    args.push("--format=%H\u{1f}%P\u{1f}%an\u{1f}%ae\u{1f}%aI\u{1f}%s".to_string());
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
        .filter_map(parse_record)
        .collect())
}

/// Total commit count for a scope, so a virtualized list can size its
/// scrollbar without ever materializing the full history.
pub async fn query_history_count(
    layer: &ProcessLayer,
    root: &Path,
    scope: &HistoryScope,
) -> Result<usize, String> {
    let mut args = vec!["rev-list".to_string(), "--count".to_string()];
    match scope {
        HistoryScope::CurrentBranch => args.push("HEAD".to_string()),
        HistoryScope::AllBranches => args.push("--branches".to_string()),
        HistoryScope::AllRefs => args.push("--all".to_string()),
        HistoryScope::Path(_) => args.push("HEAD".to_string()),
    }
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
    result
        .stdout_utf8_lossy()
        .trim()
        .parse::<usize>()
        .map_err(|e| e.to_string())
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

    async fn page(
        layer: &ProcessLayer,
        root: &Path,
        scope: &HistoryScope,
        skip: usize,
        limit: usize,
    ) -> Vec<CommitSummary> {
        query_history_page(layer, root, scope, skip, limit)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn current_branch_returns_commits_newest_first() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "first");
        commit(dir.path(), "b.txt", "second");
        commit(dir.path(), "c.txt", "third");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let commits = page(&layer, dir.path(), &HistoryScope::CurrentBranch, 0, 10).await;

        let subjects: Vec<&str> = commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["third", "second", "first"]);
    }

    #[tokio::test]
    async fn pagination_is_stable_and_covers_the_whole_range_without_gaps_or_overlap() {
        let dir = init_repo();
        for i in 0..5 {
            commit(dir.path(), &format!("f{i}.txt"), &format!("commit {i}"));
        }

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let full = page(&layer, dir.path(), &HistoryScope::CurrentBranch, 0, 100).await;

        let mut paged = Vec::new();
        for skip in [0, 2, 4] {
            paged.extend(page(&layer, dir.path(), &HistoryScope::CurrentBranch, skip, 2).await);
        }

        assert_eq!(full, paged);
    }

    #[tokio::test]
    async fn all_branches_includes_commits_from_every_local_branch() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "on main");
        git(dir.path(), &["checkout", "-q", "-b", "feature"]);
        commit(dir.path(), "b.txt", "on feature");
        git(dir.path(), &["checkout", "-q", "main"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let current = page(&layer, dir.path(), &HistoryScope::CurrentBranch, 0, 10).await;
        let all = page(&layer, dir.path(), &HistoryScope::AllBranches, 0, 10).await;

        assert_eq!(current.len(), 1);
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn all_refs_reaches_commits_only_a_tag_points_to() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "on main");
        git(dir.path(), &["checkout", "-q", "--orphan", "orphaned"]);
        commit(dir.path(), "b.txt", "orphan commit");
        git(dir.path(), &["tag", "isolated-tag"]);
        git(dir.path(), &["checkout", "-q", "main"]);
        git(dir.path(), &["branch", "-D", "orphaned"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let branches_only = page(&layer, dir.path(), &HistoryScope::AllBranches, 0, 10).await;
        let all_refs = page(&layer, dir.path(), &HistoryScope::AllRefs, 0, 10).await;

        assert_eq!(branches_only.len(), 1);
        assert_eq!(all_refs.len(), 2);
    }

    #[tokio::test]
    async fn path_scope_only_returns_commits_touching_that_path() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "touches a");
        commit(dir.path(), "b.txt", "touches b");
        std::fs::write(dir.path().join("a.txt"), "changed again").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "touches a again"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let commits = page(
            &layer,
            dir.path(),
            &HistoryScope::Path("a.txt".to_string()),
            0,
            10,
        )
        .await;

        let subjects: Vec<&str> = commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["touches a again", "touches a"]);
    }

    #[tokio::test]
    async fn parent_shas_and_author_fields_are_parsed_correctly() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "root commit");
        commit(dir.path(), "b.txt", "second commit");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let commits = page(&layer, dir.path(), &HistoryScope::CurrentBranch, 0, 10).await;

        assert_eq!(commits[0].author_name, "Test");
        assert_eq!(commits[0].author_email, "test@example.com");
        assert_eq!(commits[0].parents.len(), 1);
        assert!(
            commits[1].parents.is_empty(),
            "the root commit has no parents"
        );
    }

    #[tokio::test]
    async fn history_count_matches_the_number_of_pages_fetched() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "on main");
        git(dir.path(), &["checkout", "-q", "-b", "feature"]);
        commit(dir.path(), "b.txt", "on feature");
        git(dir.path(), &["checkout", "-q", "main"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let current_count = query_history_count(&layer, dir.path(), &HistoryScope::CurrentBranch)
            .await
            .unwrap();
        let all_count = query_history_count(&layer, dir.path(), &HistoryScope::AllBranches)
            .await
            .unwrap();

        assert_eq!(current_count, 1);
        assert_eq!(all_count, 2);
    }
}
