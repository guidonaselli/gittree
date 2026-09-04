use std::path::Path;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<String>,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FileDiff {
    pub header_lines: Vec<String>,
    pub hunks: Vec<Hunk>,
    pub is_binary: bool,
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    // "@@ -old_start,old_lines +new_start,new_lines @@ optional context"
    let body = line.strip_prefix("@@ -")?;
    let (ranges, _) = body.split_once(" @@")?;
    let (old, new) = ranges.split_once(" +")?;
    let (old_start, old_lines) = match old.split_once(',') {
        Some((s, l)) => (s.parse().ok()?, l.parse().ok()?),
        None => (old.parse().ok()?, 1),
    };
    let (new_start, new_lines) = match new.split_once(',') {
        Some((s, l)) => (s.parse().ok()?, l.parse().ok()?),
        None => (new.parse().ok()?, 1),
    };
    Some((old_start, old_lines, new_start, new_lines))
}

/// Parses `git diff`/`git diff --cached` output already scoped to one path.
pub fn parse_file_diff(text: &str) -> Option<FileDiff> {
    if text.is_empty() {
        return None;
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut header_lines = Vec::new();
    while i < lines.len() && !lines[i].starts_with("@@") {
        header_lines.push(lines[i].to_string());
        i += 1;
    }
    let is_binary = header_lines.iter().any(|l| l.starts_with("Binary files"));

    let mut hunks = Vec::new();
    while i < lines.len() {
        if let Some((old_start, old_lines, new_start, new_lines)) = parse_hunk_header(lines[i]) {
            let header = lines[i].to_string();
            i += 1;
            let mut body = Vec::new();
            while i < lines.len() && !lines[i].starts_with("@@") {
                body.push(lines[i].to_string());
                i += 1;
            }
            hunks.push(Hunk {
                header,
                lines: body,
                old_start,
                old_lines,
                new_start,
                new_lines,
            });
        } else {
            i += 1;
        }
    }

    Some(FileDiff {
        header_lines,
        hunks,
        is_binary,
    })
}

pub async fn diff_file(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    staged: bool,
) -> Result<Option<FileDiff>, String> {
    let mut args = vec!["diff".to_string()];
    if staged {
        args.push("--cached".to_string());
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
        return Err(format!("git diff failed: {}", result.stderr.trim()));
    }
    Ok(parse_file_diff(&result.stdout_utf8_lossy()))
}

/// Builds a standalone patch from a subset of a file's hunks, kept in original order.
pub fn build_patch(file: &FileDiff, hunk_indices: &[usize]) -> String {
    let mut out = file.header_lines.join("\n");
    out.push('\n');
    for &idx in hunk_indices {
        if let Some(hunk) = file.hunks.get(idx) {
            out.push_str(&hunk.header);
            out.push('\n');
            for line in &hunk.lines {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

/// Builds a single-hunk patch from a subset of its changed lines; an unselected removed line becomes context, an unselected added line is dropped.
pub fn build_partial_patch(
    file: &FileDiff,
    hunk_index: usize,
    selected_line_indices: &[usize],
) -> Option<String> {
    let hunk = file.hunks.get(hunk_index)?;
    let selected: std::collections::HashSet<usize> =
        selected_line_indices.iter().copied().collect();

    let mut body = Vec::new();
    let mut old_lines = 0u32;
    let mut new_lines = 0u32;
    for (i, line) in hunk.lines.iter().enumerate() {
        if let Some(rest) = line.strip_prefix(' ') {
            body.push(format!(" {rest}"));
            old_lines += 1;
            new_lines += 1;
        } else if let Some(rest) = line.strip_prefix('-') {
            if selected.contains(&i) {
                body.push(format!("-{rest}"));
                old_lines += 1;
            } else {
                body.push(format!(" {rest}"));
                old_lines += 1;
                new_lines += 1;
            }
        } else if let Some(rest) = line.strip_prefix('+') {
            if selected.contains(&i) {
                body.push(format!("+{rest}"));
                new_lines += 1;
            }
        } else {
            body.push(line.clone());
        }
    }

    let header = format!(
        "@@ -{},{} +{},{} @@",
        hunk.old_start, old_lines, hunk.new_start, new_lines
    );
    let mut out = file.header_lines.join("\n");
    out.push('\n');
    out.push_str(&header);
    out.push('\n');
    for l in &body {
        out.push_str(l);
        out.push('\n');
    }
    Some(out)
}

async fn apply_patch(
    layer: &ProcessLayer,
    root: &Path,
    patch: String,
    reverse: bool,
) -> Result<(), String> {
    let mut args = vec!["apply".to_string(), "--cached".to_string()];
    if reverse {
        args.push("--reverse".to_string());
    }
    let call = GitCall::new(root, args).with_stdin(patch.into_bytes());
    let result = layer
        .run(call, Intent::Write, CancellationToken::new())
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git apply failed: {}", result.stderr.trim()));
    }
    Ok(())
}

pub async fn stage_hunks(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    hunk_indices: &[usize],
) -> Result<(), String> {
    let file = diff_file(layer, root, path, false)
        .await?
        .ok_or_else(|| format!("no unstaged diff for {path}"))?;
    apply_patch(layer, root, build_patch(&file, hunk_indices), false).await
}

pub async fn unstage_hunks(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    hunk_indices: &[usize],
) -> Result<(), String> {
    let file = diff_file(layer, root, path, true)
        .await?
        .ok_or_else(|| format!("no staged diff for {path}"))?;
    apply_patch(layer, root, build_patch(&file, hunk_indices), true).await
}

pub async fn stage_lines(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    hunk_index: usize,
    line_indices: &[usize],
) -> Result<(), String> {
    if line_indices.is_empty() {
        return Err("no lines selected".to_string());
    }
    let file = diff_file(layer, root, path, false)
        .await?
        .ok_or_else(|| format!("no unstaged diff for {path}"))?;
    let patch = build_partial_patch(&file, hunk_index, line_indices)
        .ok_or_else(|| "hunk not found for this selection".to_string())?;
    apply_patch(layer, root, patch, false).await
}

pub async fn unstage_lines(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    hunk_index: usize,
    line_indices: &[usize],
) -> Result<(), String> {
    if line_indices.is_empty() {
        return Err("no lines selected".to_string());
    }
    let file = diff_file(layer, root, path, true)
        .await?
        .ok_or_else(|| format!("no staged diff for {path}"))?;
    let patch = build_partial_patch(&file, hunk_index, line_indices)
        .ok_or_else(|| "hunk not found for this selection".to_string())?;
    apply_patch(layer, root, patch, true).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::working_copy::{query_working_copy_status, ChangeCode};
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

    const SAMPLE_DIFF: &str = "diff --git a/f.txt b/f.txt\nindex aaa..bbb 100644\n--- a/f.txt\n+++ b/f.txt\n@@ -1,3 +1,3 @@\n line1\n-line2\n+line2 changed\n line3\n@@ -10,2 +10,3 @@\n line10\n+line11 added\n line12\n";

    #[test]
    fn parses_two_hunks_with_correct_headers() {
        let file = parse_file_diff(SAMPLE_DIFF).unwrap();
        assert_eq!(file.hunks.len(), 2);
        assert_eq!(file.hunks[0].old_start, 1);
        assert_eq!(file.hunks[0].old_lines, 3);
        assert_eq!(file.hunks[1].new_start, 10);
        assert_eq!(file.hunks[1].new_lines, 3);
        assert!(!file.is_binary);
    }

    #[test]
    fn building_a_patch_from_one_hunk_omits_the_other() {
        let file = parse_file_diff(SAMPLE_DIFF).unwrap();
        let patch = build_patch(&file, &[0]);
        assert!(patch.contains("@@ -1,3 +1,3 @@"));
        assert!(!patch.contains("@@ -10,2 +10,3 @@"));
        assert!(patch.contains("line2 changed"));
    }

    #[test]
    fn a_partial_patch_keeps_an_unselected_removal_as_context() {
        let file = parse_file_diff(SAMPLE_DIFF).unwrap();
        // hunk 0 lines: [0]=" line1" [1]="-line2" [2]="+line2 changed" [3]=" line3"
        let patch = build_partial_patch(&file, 0, &[2]).unwrap();
        assert!(patch.contains("@@ -1,3 +1,4 @@"));
        assert!(patch.contains(" line2"));
        assert!(!patch.contains("-line2"));
        assert!(patch.contains("+line2 changed"));
    }

    #[test]
    fn a_partial_patch_drops_an_unselected_addition_entirely() {
        let file = parse_file_diff(SAMPLE_DIFF).unwrap();
        let patch = build_partial_patch(&file, 0, &[1]).unwrap();
        assert!(patch.contains("@@ -1,3 +1,2 @@"));
        assert!(patch.contains("-line2"));
        assert!(!patch.contains("+line2 changed"));
    }

    #[tokio::test]
    async fn staging_one_line_out_of_two_pairs_in_the_same_hunk() {
        let dir = init_repo();
        let content: String = (1..=10).map(|n| format!("line{n}\n")).collect();
        std::fs::write(dir.path().join("f.txt"), &content).unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
        lines[1] = "line2 changed".to_string();
        lines[2] = "line3 changed".to_string();
        std::fs::write(dir.path().join("f.txt"), lines.join("\n") + "\n").unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let file = diff_file(&layer, dir.path(), "f.txt", false)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(file.hunks.len(), 1);
        let line2_add = file.hunks[0]
            .lines
            .iter()
            .position(|l| l == "+line2 changed")
            .unwrap();

        stage_lines(&layer, dir.path(), "f.txt", 0, &[line2_add])
            .await
            .unwrap();

        let staged = diff_file(&layer, dir.path(), "f.txt", true)
            .await
            .unwrap()
            .unwrap();
        assert!(staged.hunks[0].lines.iter().any(|l| l == "+line2 changed"));
        assert!(!staged.hunks[0]
            .lines
            .iter()
            .any(|l| l.contains("line3 changed")));

        let unstaged = diff_file(&layer, dir.path(), "f.txt", false)
            .await
            .unwrap()
            .unwrap();
        assert!(unstaged.hunks[0]
            .lines
            .iter()
            .any(|l| l.contains("line3 changed")));
        assert!(!unstaged.hunks[0]
            .lines
            .iter()
            .any(|l| l == "+line2 changed"));
    }

    #[tokio::test]
    async fn an_empty_line_selection_is_refused_not_a_silent_no_op() {
        let dir = init_repo();
        std::fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        std::fs::write(dir.path().join("f.txt"), "a\nb changed\nc\n").unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let result = stage_lines(&layer, dir.path(), "f.txt", 0, &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn staging_one_hunk_leaves_the_other_unstaged() {
        let dir = init_repo();
        let content: String = (1..=20).map(|n| format!("line{n}\n")).collect();
        std::fs::write(dir.path().join("f.txt"), &content).unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
        lines[1] = "line2 changed".to_string();
        lines[11] = "line12 changed".to_string();
        std::fs::write(dir.path().join("f.txt"), lines.join("\n") + "\n").unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let file = diff_file(&layer, dir.path(), "f.txt", false)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(file.hunks.len(), 2);

        stage_hunks(&layer, dir.path(), "f.txt", &[0])
            .await
            .unwrap();

        let status = query_working_copy_status(&layer, dir.path())
            .await
            .into_known()
            .unwrap();
        let entry = status
            .changed
            .iter()
            .find(|e| e.path.as_path() == Path::new("f.txt"))
            .unwrap();
        assert_eq!(entry.staged, ChangeCode::Modified);
        assert_eq!(entry.unstaged, ChangeCode::Modified);

        let staged_diff = diff_file(&layer, dir.path(), "f.txt", true)
            .await
            .unwrap()
            .unwrap();
        assert!(staged_diff
            .hunks
            .iter()
            .any(|h| h.lines.iter().any(|l| l.contains("line2 changed"))));
        assert!(!staged_diff
            .hunks
            .iter()
            .any(|h| h.lines.iter().any(|l| l.contains("line12 changed"))));
    }

    #[tokio::test]
    async fn unstaging_one_hunk_keeps_the_other_staged() {
        let dir = init_repo();
        let content: String = (1..=20).map(|n| format!("line{n}\n")).collect();
        std::fs::write(dir.path().join("f.txt"), &content).unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
        lines[1] = "line2 changed".to_string();
        lines[11] = "line12 changed".to_string();
        std::fs::write(dir.path().join("f.txt"), lines.join("\n") + "\n").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let staged_before = diff_file(&layer, dir.path(), "f.txt", true)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(staged_before.hunks.len(), 2);

        unstage_hunks(&layer, dir.path(), "f.txt", &[0])
            .await
            .unwrap();

        let staged_after = diff_file(&layer, dir.path(), "f.txt", true)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(staged_after.hunks.len(), 1);
        assert!(staged_after
            .hunks
            .iter()
            .any(|h| h.lines.iter().any(|l| l.contains("line12 changed"))));

        let unstaged_after = diff_file(&layer, dir.path(), "f.txt", false)
            .await
            .unwrap()
            .unwrap();
        assert!(unstaged_after
            .hunks
            .iter()
            .any(|h| h.lines.iter().any(|l| l.contains("line2 changed"))));
    }

    #[tokio::test]
    async fn a_whitespace_only_change_is_staged_byte_exact() {
        let dir = init_repo();
        std::fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        std::fs::write(dir.path().join("f.txt"), "line1\nline2  \nline3\n").unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let file = diff_file(&layer, dir.path(), "f.txt", false)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(file.hunks.len(), 1);
        assert!(file.hunks[0].lines.iter().any(|l| l == "-line2"));
        assert!(file.hunks[0].lines.iter().any(|l| l == "+line2  "));

        stage_hunks(&layer, dir.path(), "f.txt", &[0])
            .await
            .unwrap();

        let staged_blob = Command::new("git")
            .args(["show", ":f.txt"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&staged_blob.stdout),
            "line1\nline2  \nline3\n"
        );
    }
}
