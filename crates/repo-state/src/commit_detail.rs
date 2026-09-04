use std::path::Path;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const FIELD_SEP: char = '\u{1f}';

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum SignatureState {
    Unsigned,
    Valid { signer: String },
    Invalid { reason: String },
    Unverifiable { reason: String },
}

fn signature_state(code: &str, signer: &str) -> SignatureState {
    match code {
        "G" | "U" => SignatureState::Valid {
            signer: signer.to_string(),
        },
        "B" => SignatureState::Invalid {
            reason: "bad signature".to_string(),
        },
        "X" => SignatureState::Invalid {
            reason: "expired signature".to_string(),
        },
        "Y" => SignatureState::Invalid {
            reason: "signed with an expired key".to_string(),
        },
        "R" => SignatureState::Invalid {
            reason: "signed with a revoked key".to_string(),
        },
        "E" => SignatureState::Unverifiable {
            reason: "signing key unavailable".to_string(),
        },
        _ => SignatureState::Unsigned,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FileStat {
    pub path: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CommitDetail {
    pub sha: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: String,
    pub decorations: Vec<String>,
    pub subject: String,
    pub body: String,
    pub signature: SignatureState,
    pub files: Vec<FileStat>,
}

async fn run(layer: &ProcessLayer, root: &Path, args: Vec<String>) -> Result<String, String> {
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
    Ok(result.stdout_utf8_lossy().into_owned())
}

fn parse_file_stats(numstat: &str) -> Vec<FileStat> {
    numstat
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let added = parts.next()?;
            let deleted = parts.next()?;
            let path = parts.next()?.to_string();
            Some(FileStat {
                path,
                additions: added.parse::<u32>().ok(),
                deletions: deleted.parse::<u32>().ok(),
            })
        })
        .collect()
}

pub async fn query_commit_detail(
    layer: &ProcessLayer,
    root: &Path,
    sha: &str,
) -> Result<CommitDetail, String> {
    let format = format!(
        "%H{FIELD_SEP}%P{FIELD_SEP}%an{FIELD_SEP}%ae{FIELD_SEP}%aI{FIELD_SEP}%cn{FIELD_SEP}%ce{FIELD_SEP}%cI{FIELD_SEP}%G?{FIELD_SEP}%GS{FIELD_SEP}%D{FIELD_SEP}%s{FIELD_SEP}%b"
    );
    let stdout = run(
        layer,
        root,
        vec![
            "show".to_string(),
            "--no-patch".to_string(),
            format!("--format={format}"),
            sha.to_string(),
        ],
    )
    .await?;

    let mut fields = stdout.trim_end_matches('\n').splitn(13, FIELD_SEP);
    let full_sha = fields.next().unwrap_or_default().to_string();
    let parents: Vec<String> = fields
        .next()
        .unwrap_or_default()
        .split(' ')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect();
    let author_name = fields.next().unwrap_or_default().to_string();
    let author_email = fields.next().unwrap_or_default().to_string();
    let author_date = fields.next().unwrap_or_default().to_string();
    let committer_name = fields.next().unwrap_or_default().to_string();
    let committer_email = fields.next().unwrap_or_default().to_string();
    let committer_date = fields.next().unwrap_or_default().to_string();
    let sig_code = fields.next().unwrap_or_default().to_string();
    let signer = fields.next().unwrap_or_default().to_string();
    let decorations: Vec<String> = fields
        .next()
        .unwrap_or_default()
        .split(", ")
        .filter(|d| !d.is_empty())
        .map(|d| d.to_string())
        .collect();
    let subject = fields.next().unwrap_or_default().to_string();
    let body = fields.next().unwrap_or_default().trim().to_string();

    let base = parents.first().map(|s| s.as_str()).unwrap_or(EMPTY_TREE);
    let numstat = run(
        layer,
        root,
        vec![
            "diff".to_string(),
            "--numstat".to_string(),
            base.to_string(),
            full_sha.clone(),
        ],
    )
    .await?;

    Ok(CommitDetail {
        sha: full_sha,
        parents,
        author_name,
        author_email,
        author_date,
        committer_name,
        committer_email,
        committer_date,
        decorations,
        subject,
        body,
        signature: signature_state(&sig_code, &signer),
        files: parse_file_stats(&numstat),
    })
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

    fn commit(dir: &Path, name: &str, content: &str, message: &str) {
        std::fs::write(dir.join(name), content).unwrap();
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", message]);
    }

    fn head_sha(dir: &Path) -> String {
        let out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[tokio::test]
    async fn root_commit_reports_full_metadata_and_file_stats() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "line1\nline2\n", "root commit");
        let sha = head_sha(dir.path());

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let detail = query_commit_detail(&layer, dir.path(), &sha).await.unwrap();

        assert_eq!(detail.sha, sha);
        assert!(detail.parents.is_empty());
        assert_eq!(detail.author_name, "Test");
        assert_eq!(detail.author_email, "test@example.com");
        assert_eq!(detail.subject, "root commit");
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].path, "a.txt");
        assert_eq!(detail.files[0].additions, Some(2));
        assert_eq!(detail.files[0].deletions, Some(0));
        assert_eq!(detail.signature, SignatureState::Unsigned);
    }

    #[tokio::test]
    async fn a_non_root_commit_reports_its_parent_and_diff_against_it() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "one\n", "first");
        commit(dir.path(), "a.txt", "one\ntwo\nthree\n", "second");
        let parent_sha = Command::new("git")
            .args(["rev-parse", "HEAD~1"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let parent_sha = String::from_utf8_lossy(&parent_sha.stdout)
            .trim()
            .to_string();
        let sha = head_sha(dir.path());

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let detail = query_commit_detail(&layer, dir.path(), &sha).await.unwrap();

        assert_eq!(detail.parents, vec![parent_sha]);
        assert_eq!(detail.files[0].additions, Some(2));
        assert_eq!(detail.files[0].deletions, Some(0));
    }

    #[tokio::test]
    async fn decorations_include_the_branch_and_a_tag_pointing_at_head() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "x", "tagged commit");
        git(dir.path(), &["tag", "v1.0"]);
        let sha = head_sha(dir.path());

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let detail = query_commit_detail(&layer, dir.path(), &sha).await.unwrap();

        assert!(detail.decorations.iter().any(|d| d.contains("main")));
        assert!(detail.decorations.iter().any(|d| d.contains("v1.0")));
    }

    #[tokio::test]
    async fn a_merge_commit_diffs_against_its_first_parent() {
        let dir = init_repo();
        commit(dir.path(), "a.txt", "on main", "on main");
        git(dir.path(), &["checkout", "-q", "-b", "feature"]);
        commit(dir.path(), "b.txt", "on feature", "on feature");
        git(dir.path(), &["checkout", "-q", "main"]);
        git(
            dir.path(),
            &["merge", "-q", "--no-ff", "-m", "merge feature", "feature"],
        );
        let sha = head_sha(dir.path());
        let first_parent = Command::new("git")
            .args(["rev-parse", "HEAD^1"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let first_parent = String::from_utf8_lossy(&first_parent.stdout)
            .trim()
            .to_string();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let detail = query_commit_detail(&layer, dir.path(), &sha).await.unwrap();

        assert_eq!(detail.parents.len(), 2);
        assert_eq!(detail.parents[0], first_parent);
        assert!(detail.files.iter().any(|f| f.path == "b.txt"));
    }

    #[tokio::test]
    async fn body_is_separated_from_the_subject() {
        let dir = init_repo();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        git(dir.path(), &["add", "-A"]);
        git(
            dir.path(),
            &[
                "commit",
                "-q",
                "-m",
                "subject line\n\nbody paragraph one.\n\nbody paragraph two.",
            ],
        );
        let sha = head_sha(dir.path());

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let detail = query_commit_detail(&layer, dir.path(), &sha).await.unwrap();

        assert_eq!(detail.subject, "subject line");
        assert!(detail.body.contains("body paragraph one."));
        assert!(detail.body.contains("body paragraph two."));
    }
}
