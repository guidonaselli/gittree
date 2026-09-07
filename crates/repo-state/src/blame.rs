use std::collections::HashMap;
use std::path::Path;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BlameLine {
    pub sha: String,
    pub author_name: String,
    pub author_email: String,
    pub author_time: i64,
    pub summary: String,
    pub orig_line_no: u32,
    pub final_line_no: u32,
    pub content: String,
    pub is_boundary: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct BlameOptions {
    pub ignore_whitespace: bool,
    /// Blame the file as of this revision instead of the working tree/HEAD; pass `"<sha>^"`
    /// to reblame from a commit's parent.
    pub rev: Option<String>,
}

#[derive(Default, Clone)]
struct CommitMeta {
    author_name: String,
    author_email: String,
    author_time: i64,
    summary: String,
    is_boundary: bool,
}

/// Parses `git blame --porcelain` output: metadata lines only appear the first time a commit
/// is seen, so later occurrences are filled in from a cache keyed by sha.
pub fn parse_blame_porcelain(text: &str) -> Vec<BlameLine> {
    let mut lines = text.lines().peekable();
    let mut cache: HashMap<String, CommitMeta> = HashMap::new();
    let mut result = Vec::new();

    while let Some(header) = lines.next() {
        let mut parts = header.split(' ');
        let Some(sha) = parts.next() else { continue };
        if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let Some(orig_line_no) = parts.next().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Some(final_line_no) = parts.next().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };

        let mut meta = cache.get(sha).cloned().unwrap_or_default();
        let mut saw_header_fields = false;
        let content = loop {
            let Some(line) = lines.next() else {
                break String::new();
            };
            if let Some(content) = line.strip_prefix('\t') {
                break content.to_string();
            }
            saw_header_fields = true;
            if let Some(v) = line.strip_prefix("author ") {
                meta.author_name = v.to_string();
            } else if let Some(v) = line.strip_prefix("author-mail ") {
                meta.author_email = v.trim_matches(['<', '>']).to_string();
            } else if let Some(v) = line.strip_prefix("author-time ") {
                meta.author_time = v.parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("summary ") {
                meta.summary = v.to_string();
            } else if line == "boundary" {
                meta.is_boundary = true;
            }
        };

        if saw_header_fields {
            cache.insert(sha.to_string(), meta.clone());
        }

        result.push(BlameLine {
            sha: sha.to_string(),
            author_name: meta.author_name,
            author_email: meta.author_email,
            author_time: meta.author_time,
            summary: meta.summary,
            orig_line_no,
            final_line_no,
            content,
            is_boundary: meta.is_boundary,
        });
    }

    result
}

pub async fn query_blame(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    options: &BlameOptions,
) -> Result<Vec<BlameLine>, String> {
    let mut args = vec!["blame".to_string(), "--porcelain".to_string()];
    if options.ignore_whitespace {
        args.push("-w".to_string());
    }
    if let Some(rev) = &options.rev {
        args.push(rev.clone());
    }
    args.push("--".to_string());
    args.push(path.to_string());
    let result = layer
        .run(
            GitCall::new(root, args),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git blame failed: {}", result.stderr.trim()));
    }
    Ok(parse_blame_porcelain(&result.stdout_utf8_lossy()))
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
        git(dir.path(), &["config", "user.email", "a@example.com"]);
        git(dir.path(), &["config", "user.name", "Alice"]);
        dir
    }

    const SAMPLE: &str = "894a51b0b8d6a2400bbba77df8907b780fa4742c 1 1 1\nauthor Alice\nauthor-mail <a@example.com>\nauthor-time 1788560095\nauthor-tz -0300\ncommitter Alice\ncommitter-mail <a@example.com>\ncommitter-time 1788560095\ncommitter-tz -0300\nsummary first commit\nboundary\nfilename f.txt\n\tline1\na27c9772bea678eeca3ba7a14d3c3b7c04bd4f01 2 2 1\nauthor Bob\nauthor-mail <b@example.com>\nauthor-time 1788560200\nauthor-tz -0300\ncommitter Bob\ncommitter-mail <b@example.com>\ncommitter-time 1788560200\ncommitter-tz -0300\nsummary second commit\nprevious 894a51b0b8d6a2400bbba77df8907b780fa4742c f.txt\nfilename f.txt\n\tline2 edited\n894a51b0b8d6a2400bbba77df8907b780fa4742c 3 3 1\n\tline3\n";

    #[test]
    fn parses_three_lines_with_correct_attribution() {
        let lines = parse_blame_porcelain(SAMPLE);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].content, "line1");
        assert_eq!(lines[0].author_name, "Alice");
        assert!(lines[0].is_boundary);
        assert_eq!(lines[1].content, "line2 edited");
        assert_eq!(lines[1].author_name, "Bob");
        assert!(!lines[1].is_boundary);
    }

    #[test]
    fn a_reused_commit_inherits_cached_metadata_not_blank_fields() {
        let lines = parse_blame_porcelain(SAMPLE);
        assert_eq!(lines[2].sha, lines[0].sha);
        assert_eq!(lines[2].author_name, "Alice");
        assert_eq!(lines[2].summary, "first commit");
        assert_eq!(lines[2].content, "line3");
    }

    #[tokio::test]
    async fn a_real_two_commit_file_blames_each_line_to_its_real_author() {
        let dir = init_repo();
        std::fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "first commit"]);
        git(dir.path(), &["config", "user.email", "b@example.com"]);
        git(dir.path(), &["config", "user.name", "Bob"]);
        std::fs::write(dir.path().join("f.txt"), "line1\nline2 edited\nline3\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "second commit"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let lines = query_blame(&layer, dir.path(), "f.txt", &BlameOptions::default())
            .await
            .unwrap();

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].author_name, "Alice");
        assert_eq!(lines[1].author_name, "Bob");
        assert_eq!(lines[2].author_name, "Alice");
    }

    #[tokio::test]
    async fn reblaming_from_a_commits_parent_attributes_the_line_to_the_earlier_author() {
        let dir = init_repo();
        std::fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "first commit"]);
        git(dir.path(), &["config", "user.email", "b@example.com"]);
        git(dir.path(), &["config", "user.name", "Bob"]);
        std::fs::write(dir.path().join("f.txt"), "line1\nline2 edited\nline3\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "second commit"]);
        let head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let head = String::from_utf8_lossy(&head.stdout).trim().to_string();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let options = BlameOptions {
            ignore_whitespace: false,
            rev: Some(format!("{head}^")),
        };
        let lines = query_blame(&layer, dir.path(), "f.txt", &options)
            .await
            .unwrap();

        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.author_name == "Alice"));
    }

    #[tokio::test]
    async fn ignore_whitespace_attributes_a_reindented_line_to_its_original_author() {
        let dir = init_repo();
        std::fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "first commit"]);
        git(dir.path(), &["config", "user.email", "b@example.com"]);
        git(dir.path(), &["config", "user.name", "Bob"]);
        std::fs::write(dir.path().join("f.txt"), "line1\n    line2\nline3\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "reindent"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let options = BlameOptions {
            ignore_whitespace: true,
            rev: None,
        };
        let lines = query_blame(&layer, dir.path(), "f.txt", &options)
            .await
            .unwrap();

        assert_eq!(lines[1].author_name, "Alice");
    }
}
