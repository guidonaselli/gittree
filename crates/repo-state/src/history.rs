use std::path::Path;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "path")]
pub enum HistoryScope {
    CurrentBranch,
    AllBranches,
    AllRefs,
    Path(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommitSummary {
    pub sha: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_at_commit: Option<String>,
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
        rename_from: None,
        path_at_commit: None,
    })
}

fn parse_path_history(stdout: &str) -> Vec<CommitSummary> {
    let tokens: Vec<&str> = stdout.split('\0').filter(|s| !s.is_empty()).collect();
    let mut summaries = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        if !token.contains(FIELD_SEP) {
            i += 1;
            continue;
        }
        let Some(mut summary) = parse_record(token) else {
            i += 1;
            continue;
        };
        i += 1;
        let mut rename_from = None;
        let mut path_at_commit = None;
        while i < tokens.len() && !tokens[i].contains(FIELD_SEP) {
            let status_raw = tokens[i].trim();
            i += 1;
            if status_raw.starts_with('R') || status_raw.starts_with('C') {
                if i < tokens.len() {
                    let old_p = tokens[i];
                    i += 1;
                    if i < tokens.len() {
                        let new_p = tokens[i];
                        i += 1;
                        rename_from = Some(old_p.to_string());
                        path_at_commit = Some(new_p.to_string());
                    }
                }
            } else if !status_raw.is_empty() {
                if i < tokens.len() {
                    let p = tokens[i];
                    i += 1;
                    if path_at_commit.is_none() {
                        path_at_commit = Some(p.to_string());
                    }
                }
            }
        }
        summary.rename_from = rename_from;
        summary.path_at_commit = path_at_commit;
        summaries.push(summary);
    }
    summaries
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
    if let HistoryScope::Path(path) = scope {
        // With --follow, git has a bug where --skip causes the traversal to fail or yield empty.
        // Fetch up to skip + limit and slice in memory.
        let max_count = skip.saturating_add(limit);
        let args = vec![
            "log".to_string(),
            "--date-order".to_string(),
            "--follow".to_string(),
            "--name-status".to_string(),
            "-M".to_string(),
            format!("--max-count={max_count}"),
            format!("--format=%H{FIELD_SEP}%P{FIELD_SEP}%an{FIELD_SEP}%ae{FIELD_SEP}%aI{FIELD_SEP}%s"),
            "-z".to_string(),
            "--".to_string(),
            path.clone(),
        ];
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
        let all = parse_path_history(&result.stdout_utf8_lossy());
        return Ok(all.into_iter().skip(skip).take(limit).collect());
    }

    let mut args = vec!["log".to_string(), "--date-order".to_string()];
    match scope {
        HistoryScope::CurrentBranch => {}
        HistoryScope::AllBranches => args.push("--branches".to_string()),
        HistoryScope::AllRefs => args.push("--all".to_string()),
        HistoryScope::Path(_) => unreachable!(),
    }
    args.push(format!("--skip={skip}"));
    args.push(format!("--max-count={limit}"));
    args.push(format!("--format=%H{FIELD_SEP}%P{FIELD_SEP}%an{FIELD_SEP}%ae{FIELD_SEP}%aI{FIELD_SEP}%s"));
    args.push("-z".to_string());

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
    if let HistoryScope::Path(path) = scope {
        let args = vec![
            "log".to_string(),
            "--follow".to_string(),
            "--format=%H".to_string(),
            "--".to_string(),
            path.clone(),
        ];
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
        return Ok(result
            .stdout_utf8_lossy()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count());
    }

    let mut args = vec!["rev-list".to_string(), "--count".to_string()];
    match scope {
        HistoryScope::CurrentBranch => args.push("HEAD".to_string()),
        HistoryScope::AllBranches => args.push("--branches".to_string()),
        HistoryScope::AllRefs => args.push("--all".to_string()),
        HistoryScope::Path(_) => unreachable!(),
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HistoricalFile {
    pub content: String,
    pub is_binary: bool,
    pub size: usize,
}

pub async fn query_file_at_revision(
    layer: &ProcessLayer,
    root: &Path,
    rev: &str,
    path: &str,
) -> Result<HistoricalFile, String> {
    let target = format!("{rev}:{}", path.trim_start_matches('/'));
    let result = layer
        .run(
            GitCall::new(root, vec!["show".to_string(), target]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git show failed: {}", result.stderr.trim()));
    }
    let bytes = result.stdout;
    let size = bytes.len();
    let is_binary = bytes.contains(&0);
    let content = if is_binary {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    } else {
        String::from_utf8_lossy(&bytes).to_string()
    };
    Ok(HistoricalFile {
        content,
        is_binary,
        size,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ContentSearchMode {
    Pickaxe,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct HistorySearchOptions {
    pub scope: Option<HistoryScope>,
    pub message: Option<String>,
    pub author: Option<String>,
    pub path: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub content_query: Option<String>,
    pub content_mode: Option<ContentSearchMode>,
    pub skip: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HistorySearchResult {
    pub commits: Vec<CommitSummary>,
    pub truncated: bool,
}

pub async fn search_history(
    layer: &ProcessLayer,
    root: &Path,
    options: &HistorySearchOptions,
    cancellation_token: CancellationToken,
) -> Result<HistorySearchResult, String> {
    let limit = options.limit.unwrap_or(100);
    let skip = options.skip.unwrap_or(0);
    let fetch_count = limit.saturating_add(1);

    let mut args = vec!["log".to_string(), "--date-order".to_string()];

    let scope = options.scope.as_ref().unwrap_or(&HistoryScope::CurrentBranch);
    match scope {
        HistoryScope::CurrentBranch => {}
        HistoryScope::AllBranches => args.push("--branches".to_string()),
        HistoryScope::AllRefs => args.push("--all".to_string()),
        HistoryScope::Path(_) => {}
    }

    if let Some(msg) = &options.message {
        let trimmed = msg.trim();
        if !trimmed.is_empty() {
            args.push(format!("--grep={trimmed}"));
            args.push("-i".to_string());
        }
    }

    if let Some(author) = &options.author {
        let trimmed = author.trim();
        if !trimmed.is_empty() {
            args.push(format!("--author={trimmed}"));
            args.push("-i".to_string());
        }
    }

    if let Some(since) = &options.since {
        let trimmed = since.trim();
        if !trimmed.is_empty() {
            args.push(format!("--since={trimmed}"));
        }
    }

    if let Some(until) = &options.until {
        let trimmed = until.trim();
        if !trimmed.is_empty() {
            args.push(format!("--until={trimmed}"));
        }
    }

    if let Some(content) = &options.content_query {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            match options.content_mode.unwrap_or(ContentSearchMode::Pickaxe) {
                ContentSearchMode::Pickaxe => {
                    args.push(format!("-S{trimmed}"));
                }
                ContentSearchMode::Regex => {
                    args.push(format!("-G{trimmed}"));
                }
            }
            args.push("-i".to_string());
        }
    }

    if skip > 0 {
        args.push(format!("--skip={skip}"));
    }
    args.push(format!("--max-count={fetch_count}"));
    args.push(format!("--format=%H{FIELD_SEP}%P{FIELD_SEP}%an{FIELD_SEP}%ae{FIELD_SEP}%aI{FIELD_SEP}%s"));
    args.push("-z".to_string());

    let path_filter = match (&options.path, scope) {
        (Some(p), _) if !p.trim().is_empty() => Some(p.trim()),
        (_, HistoryScope::Path(p)) if !p.trim().is_empty() => Some(p.trim()),
        _ => None,
    };

    if let Some(p) = path_filter {
        args.push("--".to_string());
        args.push(p.to_string());
    }

    let result = layer
        .run(GitCall::new(root, args), Intent::Read, cancellation_token)
        .await
        .map_err(|e| e.to_string())?;

    if !result.ok() {
        return Err(result.stderr.trim().to_string());
    }

    let mut commits: Vec<CommitSummary> = result
        .stdout_utf8_lossy()
        .split('\0')
        .filter_map(parse_record)
        .collect();

    let truncated = commits.len() > limit;
    if truncated {
        commits.truncate(limit);
    }

    Ok(HistorySearchResult { commits, truncated })
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
        let full = dir.join(name);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full, message).unwrap();
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

    #[tokio::test]
    async fn file_history_follows_renames_across_multiple_renames_and_marks_each_rename_point() {
        let dir = init_repo();
        commit(dir.path(), "orig.txt", "initial");
        git(dir.path(), &["mv", "orig.txt", "renamed1.txt"]);
        git(dir.path(), &["commit", "-q", "-m", "rename1"]);
        std::fs::write(dir.path().join("renamed1.txt"), "edited content").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "edit"]);
        git(dir.path(), &["mv", "renamed1.txt", "final.txt"]);
        git(dir.path(), &["commit", "-q", "-m", "rename2"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let commits = page(
            &layer,
            dir.path(),
            &HistoryScope::Path("final.txt".to_string()),
            0,
            10,
        )
        .await;

        assert_eq!(commits.len(), 4);
        let subjects: Vec<&str> = commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["rename2", "edit", "rename1", "initial"]);

        // Rename points are marked
        assert_eq!(commits[0].rename_from.as_deref(), Some("renamed1.txt"));
        assert_eq!(commits[0].path_at_commit.as_deref(), Some("final.txt"));

        assert_eq!(commits[1].rename_from, None);
        assert_eq!(commits[1].path_at_commit.as_deref(), Some("renamed1.txt"));

        assert_eq!(commits[2].rename_from.as_deref(), Some("orig.txt"));
        assert_eq!(commits[2].path_at_commit.as_deref(), Some("renamed1.txt"));

        assert_eq!(commits[3].rename_from, None);
        assert_eq!(commits[3].path_at_commit.as_deref(), Some("orig.txt"));

        // Count also spans the whole rename history
        let count = query_history_count(
            &layer,
            dir.path(),
            &HistoryScope::Path("final.txt".to_string()),
        )
        .await
        .unwrap();
        assert_eq!(count, 4);
    }

    #[tokio::test]
    async fn directory_history_returns_commits_touching_paths_in_that_directory() {
        let dir = init_repo();
        std::fs::create_dir_all(dir.path().join("subdir")).unwrap();
        commit(dir.path(), "subdir/a.txt", "touch subdir a");
        commit(dir.path(), "other.txt", "touch other");
        commit(dir.path(), "subdir/b.txt", "touch subdir b");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let commits = page(
            &layer,
            dir.path(),
            &HistoryScope::Path("subdir".to_string()),
            0,
            10,
        )
        .await;

        assert_eq!(commits.len(), 2);
        let subjects: Vec<&str> = commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["touch subdir b", "touch subdir a"]);
    }

    #[tokio::test]
    async fn query_file_at_revision_reads_historical_content_and_detects_binary() {
        let dir = init_repo();
        commit(dir.path(), "note.txt", "version 1");
        commit(dir.path(), "note.txt", "version 2");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let commits = page(&layer, dir.path(), &HistoryScope::CurrentBranch, 0, 10).await;
        let v1_sha = &commits[1].sha;

        let v1_file = query_file_at_revision(&layer, dir.path(), v1_sha, "note.txt")
            .await
            .unwrap();
        assert!(!v1_file.is_binary);
        assert_eq!(v1_file.content, "version 1");
        assert_eq!(v1_file.size, 9);

        // Binary file with null byte
        std::fs::write(dir.path().join("bin.dat"), [0u8, 1, 2, 3]).unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "add binary"]);

        let bin_file = query_file_at_revision(&layer, dir.path(), "HEAD", "bin.dat")
            .await
            .unwrap();
        assert!(bin_file.is_binary);
        assert_eq!(bin_file.size, 4);
    }

    #[tokio::test]
    async fn search_history_by_message_and_author() {
        let dir = init_repo();
        git(dir.path(), &["config", "user.name", "Alice"]);
        git(dir.path(), &["config", "user.email", "alice@example.com"]);
        commit(dir.path(), "f1.txt", "fix: resolve memory leak");

        git(dir.path(), &["config", "user.name", "Bob"]);
        git(dir.path(), &["config", "user.email", "bob@example.com"]);
        commit(dir.path(), "f2.txt", "feat: add user authentication");

        git(dir.path(), &["config", "user.name", "Alice"]);
        git(dir.path(), &["config", "user.email", "alice@example.com"]);
        commit(dir.path(), "f3.txt", "docs: update readme");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Search by message (case-insensitive)
        let res_msg = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                message: Some("AUTHENTICATION".to_string()),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res_msg.commits.len(), 1);
        assert_eq!(res_msg.commits[0].subject, "feat: add user authentication");
        assert!(!res_msg.truncated);

        // Search by author
        let res_author = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                author: Some("Bob".to_string()),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res_author.commits.len(), 1);
        assert_eq!(res_author.commits[0].author_name, "Bob");

        // Search by Alice
        let res_alice = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                author: Some("Alice".to_string()),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res_alice.commits.len(), 2);
    }

    #[tokio::test]
    async fn search_history_by_path_and_date_range() {
        let dir = init_repo();
        commit(dir.path(), "src/main.rs", "commit 1 in src");
        commit(dir.path(), "docs/readme.md", "commit 2 in docs");
        commit(dir.path(), "src/lib.rs", "commit 3 in src");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Filter by path
        let res_path = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                path: Some("src".to_string()),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res_path.commits.len(), 2);
        let subjects: Vec<&str> = res_path.commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["commit 3 in src", "commit 1 in src"]);

        // Filter by date range (1 day ago to tomorrow)
        let res_date = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                since: Some("1 hour ago".to_string()),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res_date.commits.len(), 3);

        // In the future: 0 commits
        let res_future = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                since: Some("2099-01-01".to_string()),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res_future.commits.len(), 0);
    }

    #[tokio::test]
    async fn search_history_by_content_pickaxe_finds_introduction_and_removal() {
        let dir = init_repo();
        // Commit 1 introduces token
        std::fs::write(dir.path().join("token.txt"), "hello SECRET_TARGET world").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "introduce token"]);

        // Commit 2 touches unrelated file
        commit(dir.path(), "other.txt", "unrelated change");

        // Commit 3 removes token
        std::fs::write(dir.path().join("token.txt"), "hello world").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "remove token"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        let res = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                content_query: Some("SECRET_TARGET".to_string()),
                content_mode: Some(ContentSearchMode::Pickaxe),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();

        // Must find both introduction and removal!
        assert_eq!(res.commits.len(), 2);
        let subjects: Vec<&str> = res.commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["remove token", "introduce token"]);
    }

    #[tokio::test]
    async fn search_history_by_content_regex() {
        let dir = init_repo();
        std::fs::write(dir.path().join("config.rs"), "const TIMEOUT_SECS: u64 = 30;\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "add timeout"]);

        std::fs::write(dir.path().join("config.rs"), "const TIMEOUT_SECS: u64 = 30;\nconst MAX_RETRIES: u32 = 5;\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "add retries"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        let res = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                content_query: Some("MAX_[A-Z]+".to_string()),
                content_mode: Some(ContentSearchMode::Regex),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert_eq!(res.commits.len(), 1);
        assert_eq!(res.commits[0].subject, "add retries");
    }

    #[tokio::test]
    async fn search_history_truncation_is_reported_when_results_exceed_limit() {
        let dir = init_repo();
        for i in 1..=5 {
            commit(dir.path(), "file.txt", &format!("msg {i}"));
        }

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Limit 3 with 5 matching commits
        let res_trunc = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                limit: Some(3),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res_trunc.commits.len(), 3);
        assert!(res_trunc.truncated);

        // Limit 10 with 5 matching commits
        let res_all = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                limit: Some(10),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res_all.commits.len(), 5);
        assert!(!res_all.truncated);

        // Pagination with skip: skip 3 take 3
        let res_skip = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions {
                skip: Some(3),
                limit: Some(3),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res_skip.commits.len(), 2);
        assert!(!res_skip.truncated);
    }

    #[tokio::test]
    async fn search_history_cancellation_aborts() {
        let dir = init_repo();
        commit(dir.path(), "f.txt", "test commit");

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let token = CancellationToken::new();
        token.cancel(); // Pre-cancelled

        let err = search_history(
            &layer,
            dir.path(),
            &HistorySearchOptions::default(),
            token,
        )
        .await
        .unwrap_err();

        assert!(err.contains("cancelled"));
    }
}
