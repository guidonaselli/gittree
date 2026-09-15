use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

use crate::resolved::Resolved;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ChangeCode {
    Unmodified,
    Modified,
    TypeChanged,
    Added,
    Deleted,
    Renamed,
    Copied,
    Unmerged,
}

fn parse_change_code(c: char) -> ChangeCode {
    match c {
        'M' => ChangeCode::Modified,
        'T' => ChangeCode::TypeChanged,
        'A' => ChangeCode::Added,
        'D' => ChangeCode::Deleted,
        'R' => ChangeCode::Renamed,
        'C' => ChangeCode::Copied,
        'U' => ChangeCode::Unmerged,
        _ => ChangeCode::Unmodified,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SubmoduleFlags {
    pub commit_changed: bool,
    pub has_tracked_changes: bool,
    pub has_untracked_changes: bool,
}

fn parse_submodule_field(sub: &str) -> Option<SubmoduleFlags> {
    let mut chars = sub.chars();
    if chars.next()? != 'S' {
        return None;
    }
    Some(SubmoduleFlags {
        commit_changed: chars.next() == Some('C'),
        has_tracked_changes: chars.next() == Some('M'),
        has_untracked_changes: chars.next() == Some('U'),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChangedEntry {
    pub path: PathBuf,
    pub staged: ChangeCode,
    pub unstaged: ChangeCode,
    pub submodule: Option<SubmoduleFlags>,
    pub rename_or_copy_from: Option<(PathBuf, u8)>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConflictEntry {
    pub path: PathBuf,
    pub code: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct WorkingCopyStatus {
    pub changed: Vec<ChangedEntry>,
    pub untracked: Vec<PathBuf>,
    pub conflicted: Vec<ConflictEntry>,
}

fn parse_ordinary(rest: &str) -> Option<ChangedEntry> {
    let f: Vec<&str> = rest.splitn(8, ' ').collect();
    if f.len() < 8 {
        return None;
    }
    let mut xy = f[0].chars();
    Some(ChangedEntry {
        path: PathBuf::from(f[7]),
        staged: parse_change_code(xy.next()?),
        unstaged: parse_change_code(xy.next()?),
        submodule: parse_submodule_field(f[1]),
        rename_or_copy_from: None,
    })
}

fn parse_renamed(rest: &str) -> Option<ChangedEntry> {
    let f: Vec<&str> = rest.splitn(9, ' ').collect();
    if f.len() < 9 {
        return None;
    }
    let mut xy = f[0].chars();
    let score: u8 = f[7].get(1..).and_then(|s| s.parse().ok()).unwrap_or(0);
    Some(ChangedEntry {
        path: PathBuf::from(f[8]),
        staged: parse_change_code(xy.next()?),
        unstaged: parse_change_code(xy.next()?),
        submodule: parse_submodule_field(f[1]),
        rename_or_copy_from: Some((PathBuf::new(), score)),
    })
}

fn parse_unmerged(rest: &str) -> Option<ConflictEntry> {
    let f: Vec<&str> = rest.splitn(10, ' ').collect();
    if f.len() < 10 {
        return None;
    }
    Some(ConflictEntry {
        path: PathBuf::from(f[9]),
        code: f[0].to_string(),
    })
}

/// Parses `git status --porcelain=v2 -z`; a rename/copy record's origPath is a second NUL field.
pub fn parse_working_copy_status(raw: &[u8]) -> WorkingCopyStatus {
    let text = String::from_utf8_lossy(raw);
    let mut fields: VecDeque<&str> = text.split('\0').collect();
    let mut out = WorkingCopyStatus::default();

    while let Some(field) = fields.pop_front() {
        if field.is_empty() {
            continue;
        }
        let mut parts = field.splitn(2, ' ');
        let kind = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("");
        match kind {
            "1" => out.changed.extend(parse_ordinary(rest)),
            "2" => {
                let orig = fields.pop_front().unwrap_or("");
                if let Some(mut entry) = parse_renamed(rest) {
                    entry.rename_or_copy_from = entry
                        .rename_or_copy_from
                        .map(|(_, score)| (PathBuf::from(orig), score));
                    out.changed.push(entry);
                }
            }
            "u" => out.conflicted.extend(parse_unmerged(rest)),
            "?" => out.untracked.push(PathBuf::from(rest)),
            _ => {}
        }
    }
    out
}

pub async fn query_working_copy_status_cancellable(
    layer: &ProcessLayer,
    root: &Path,
    cancel: CancellationToken,
) -> Resolved<WorkingCopyStatus> {
    match layer
        .run(
            GitCall::new(
                root,
                ["status", "--porcelain=v2", "-z", "--untracked-files=all"],
            ),
            Intent::Read,
            cancel,
        )
        .await
    {
        Ok(r) if r.ok() => Resolved::known(parse_working_copy_status(&r.stdout)),
        Ok(r) => Resolved::unknown(format!("status failed: {}", r.stderr.trim())),
        Err(git_process::GitError::Cancelled) => Resolved::unknown("cancelled"),
        Err(e) => Resolved::unknown(e.to_string()),
    }
}

pub async fn query_working_copy_status(
    layer: &ProcessLayer,
    root: &Path,
) -> Resolved<WorkingCopyStatus> {
    query_working_copy_status_cancellable(layer, root, CancellationToken::new()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .status()
                .unwrap();
            assert!(status.success());
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        dir
    }

    fn commit_all(dir: &Path, msg: &str) {
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(dir)
                .status()
                .unwrap();
            assert!(status.success());
        };
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", msg]);
    }

    #[test]
    fn parses_a_staged_add() {
        let raw = b"1 A. N... 000000 100644 100644 0000000000000000000000000000000000000000 abc123 file.txt\0";
        let status = parse_working_copy_status(raw);
        assert_eq!(status.changed.len(), 1);
        assert_eq!(status.changed[0].staged, ChangeCode::Added);
        assert_eq!(status.changed[0].unstaged, ChangeCode::Unmodified);
        assert_eq!(status.changed[0].path, PathBuf::from("file.txt"));
    }

    #[test]
    fn parses_a_rename_with_similarity_and_the_origin_path() {
        let raw = b"2 R. N... 100644 100644 100644 aaa bbb R087 new-name.txt\0old-name.txt\0";
        let status = parse_working_copy_status(raw);
        assert_eq!(status.changed.len(), 1);
        let entry = &status.changed[0];
        assert_eq!(entry.path, PathBuf::from("new-name.txt"));
        assert_eq!(entry.staged, ChangeCode::Renamed);
        assert_eq!(
            entry.rename_or_copy_from,
            Some((PathBuf::from("old-name.txt"), 87))
        );
    }

    #[test]
    fn parses_an_unmerged_conflict_and_a_trailing_untracked_file() {
        let raw = b"u UU N... 100644 100644 100644 100644 aaa bbb ccc conflicted.txt\0? new.txt\0";
        let status = parse_working_copy_status(raw);
        assert_eq!(
            status.conflicted,
            vec![ConflictEntry {
                path: PathBuf::from("conflicted.txt"),
                code: "UU".to_string(),
            }]
        );
        assert_eq!(status.untracked, vec![PathBuf::from("new.txt")]);
    }

    #[test]
    fn parses_a_dirty_submodule() {
        let raw = b"1 .M SC.U 160000 160000 160000 aaa bbb sub\0";
        let status = parse_working_copy_status(raw);
        let flags = status.changed[0].submodule.unwrap();
        assert!(flags.commit_changed);
        assert!(!flags.has_tracked_changes);
        assert!(flags.has_untracked_changes);
    }

    #[test]
    fn a_path_with_an_embedded_space_is_kept_whole() {
        let raw = b"? has space.txt\0";
        let status = parse_working_copy_status(raw);
        assert_eq!(status.untracked, vec![PathBuf::from("has space.txt")]);
    }

    #[tokio::test]
    async fn a_real_rename_is_detected_end_to_end() {
        let dir = init_repo();
        std::fs::write(
            dir.path().join("old.txt"),
            "content that is long enough to be similar",
        )
        .unwrap();
        commit_all(dir.path(), "add old.txt");
        std::fs::rename(dir.path().join("old.txt"), dir.path().join("new.txt")).unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let status = query_working_copy_status(&layer, dir.path())
            .await
            .into_known()
            .unwrap();
        let entry = status
            .changed
            .iter()
            .find(|e| e.path.as_path() == Path::new("new.txt"))
            .expect("expected the renamed entry");
        assert_eq!(entry.staged, ChangeCode::Renamed);
        assert_eq!(
            entry.rename_or_copy_from.as_ref().map(|(p, _)| p.clone()),
            Some(PathBuf::from("old.txt"))
        );
    }

    #[tokio::test]
    async fn a_real_conflict_is_reported_not_invented() {
        let dir = init_repo();
        std::fs::write(dir.path().join("f.txt"), "base").unwrap();
        commit_all(dir.path(), "base");
        Command::new("git")
            .args(["checkout", "-qb", "other"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        std::fs::write(dir.path().join("f.txt"), "other side").unwrap();
        commit_all(dir.path(), "other side");
        Command::new("git")
            .args(["checkout", "-q", "main"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        std::fs::write(dir.path().join("f.txt"), "main side").unwrap();
        commit_all(dir.path(), "main side");
        let _ = Command::new("git")
            .args(["merge", "-q", "other"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let status = query_working_copy_status(&layer, dir.path())
            .await
            .into_known()
            .unwrap();
        assert_eq!(status.conflicted.len(), 1);
        assert_eq!(status.conflicted[0].path, PathBuf::from("f.txt"));
    }

    #[tokio::test]
    async fn working_copy_status_cancellation_aborts() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let cancel = CancellationToken::new();
        cancel.cancel();
        let status = query_working_copy_status_cancellable(&layer, dir.path(), cancel).await;
        assert_eq!(status, Resolved::unknown("cancelled"));
    }
}
