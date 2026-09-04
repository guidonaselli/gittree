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
pub enum NonTextualDiff {
    Binary {
        old_size: Option<u64>,
        new_size: Option<u64>,
        old_sha: Option<String>,
        new_sha: Option<String>,
    },
    Submodule {
        old_commit: Option<String>,
        new_commit: Option<String>,
    },
    Symlink {
        old_target: Option<String>,
        new_target: Option<String>,
    },
    ModeOnly {
        old_mode: String,
        new_mode: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FileDiff {
    pub header_lines: Vec<String>,
    pub hunks: Vec<Hunk>,
    pub is_binary: bool,
    pub non_textual: Option<NonTextualDiff>,
}

/// Parses the two blob shas from an `index <old>..<new>[ <mode>]` header line, if present.
fn index_line_shas(header_lines: &[String]) -> Option<(String, String)> {
    for line in header_lines {
        let Some(rest) = line.strip_prefix("index ") else {
            continue;
        };
        let shas = rest.split(' ').next()?;
        let Some((old_sha, new_sha)) = shas.split_once("..") else {
            continue;
        };
        return Some((old_sha.to_string(), new_sha.to_string()));
    }
    None
}

/// Finds the object mode wherever git puts it: trailing the `index` line, or on its own
/// `new file mode`/`deleted file mode` line for an added or removed file.
fn object_mode(header_lines: &[String]) -> Option<String> {
    for line in header_lines {
        if let Some(rest) = line.strip_prefix("index ") {
            if let Some((_, mode)) = rest.split_once(' ') {
                return Some(mode.to_string());
            }
        }
    }
    for line in header_lines {
        if let Some(mode) = line
            .strip_prefix("new file mode ")
            .or_else(|| line.strip_prefix("deleted file mode "))
        {
            return Some(mode.to_string());
        }
    }
    None
}

fn mode_only_change(header_lines: &[String]) -> Option<(String, String)> {
    let old_mode = header_lines
        .iter()
        .find_map(|l| l.strip_prefix("old mode "))?
        .to_string();
    let new_mode = header_lines
        .iter()
        .find_map(|l| l.strip_prefix("new mode "))?
        .to_string();
    Some((old_mode, new_mode))
}

fn zero_sha_to_none(sha: &str) -> Option<String> {
    if sha.chars().all(|c| c == '0') {
        None
    } else {
        Some(sha.to_string())
    }
}

fn strip_first_hunk_sides(
    hunks: &[Hunk],
    prefix_to_strip: &str,
) -> (Option<String>, Option<String>) {
    let Some(hunk) = hunks.first() else {
        return (None, None);
    };
    let old = hunk
        .lines
        .iter()
        .find_map(|l| l.strip_prefix('-'))
        .and_then(|l| l.strip_prefix(prefix_to_strip))
        .map(str::to_string);
    let new = hunk
        .lines
        .iter()
        .find_map(|l| l.strip_prefix('+'))
        .and_then(|l| l.strip_prefix(prefix_to_strip))
        .map(str::to_string);
    (old, new)
}

fn classify_non_textual(
    header_lines: &[String],
    hunks: &[Hunk],
    is_binary: bool,
) -> Option<NonTextualDiff> {
    if let Some((old_mode, new_mode)) = mode_only_change(header_lines) {
        if hunks.is_empty() && !is_binary {
            return Some(NonTextualDiff::ModeOnly { old_mode, new_mode });
        }
    }
    if let Some(mode) = object_mode(header_lines) {
        match mode.as_str() {
            "160000" => {
                let (old_commit, new_commit) = strip_first_hunk_sides(hunks, "Subproject commit ");
                return Some(NonTextualDiff::Submodule {
                    old_commit,
                    new_commit,
                });
            }
            "120000" => {
                let (old_target, new_target) = strip_first_hunk_sides(hunks, "");
                return Some(NonTextualDiff::Symlink {
                    old_target,
                    new_target,
                });
            }
            _ if is_binary => {
                let shas = index_line_shas(header_lines);
                let old_sha = shas.as_ref().and_then(|(o, _)| zero_sha_to_none(o));
                let new_sha = shas.as_ref().and_then(|(_, n)| zero_sha_to_none(n));
                return Some(NonTextualDiff::Binary {
                    old_size: None,
                    new_size: None,
                    old_sha,
                    new_sha,
                });
            }
            _ => {}
        }
    }
    None
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

    let non_textual = classify_non_textual(&header_lines, &hunks, is_binary);
    Some(FileDiff {
        header_lines,
        hunks,
        is_binary,
        non_textual,
    })
}

/// View-only diff options; the staging path never applies these.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DiffViewOptions {
    pub context_lines: Option<u32>,
    pub ignore_whitespace: bool,
}

pub async fn diff_file(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    staged: bool,
) -> Result<Option<FileDiff>, String> {
    diff_file_with_options(layer, root, path, staged, &DiffViewOptions::default()).await
}

pub async fn diff_file_with_options(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    staged: bool,
    options: &DiffViewOptions,
) -> Result<Option<FileDiff>, String> {
    let mut args = vec!["diff".to_string()];
    if staged {
        args.push("--cached".to_string());
    }
    if let Some(context) = options.context_lines {
        args.push(format!("-U{context}"));
    }
    if options.ignore_whitespace {
        args.push("--ignore-all-space".to_string());
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
    let mut file = parse_file_diff(&result.stdout_utf8_lossy());
    if let Some(f) = &mut file {
        if let Some(NonTextualDiff::Binary {
            old_sha, new_sha, ..
        }) = f.non_textual.clone()
        {
            let old_size = blob_size(layer, root, old_sha.clone()).await;
            // The worktree side of an unstaged diff isn't a real object in the store —
            // git only computes that hash for display, so `cat-file` can't read it back.
            let new_size = if staged {
                blob_size(layer, root, new_sha.clone()).await
            } else {
                tokio::fs::metadata(root.join(path))
                    .await
                    .ok()
                    .map(|m| m.len())
            };
            f.non_textual = Some(NonTextualDiff::Binary {
                old_size,
                new_size,
                old_sha,
                new_sha,
            });
        }
    }
    Ok(file)
}

/// Reads a blob's raw bytes and base64-encodes them, for previewing an image-like binary side.
pub async fn read_blob_base64(
    layer: &ProcessLayer,
    root: &Path,
    sha: &str,
) -> Result<String, String> {
    use base64::Engine;
    let result = layer
        .run(
            GitCall::new(
                root,
                vec!["cat-file".to_string(), "blob".to_string(), sha.to_string()],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| e.to_string())?;
    if !result.ok() {
        return Err(format!("git cat-file failed: {}", result.stderr.trim()));
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(&result.stdout))
}

/// Reads a working-tree file's raw bytes and base64-encodes them. Used for the unstaged side of
/// an image preview, since an unstaged file has no real blob object `read_blob_base64` could fetch.
pub async fn read_working_tree_file_base64(root: &Path, path: &str) -> Result<String, String> {
    use base64::Engine;
    let bytes = tokio::fs::read(root.join(path))
        .await
        .map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

async fn blob_size(layer: &ProcessLayer, root: &Path, sha: Option<String>) -> Option<u64> {
    let sha = sha?;
    let result = layer
        .run(
            GitCall::new(root, vec!["cat-file".to_string(), "-s".to_string(), sha]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .ok()?;
    if !result.ok() {
        return None;
    }
    result.stdout_utf8_lossy().trim().parse().ok()
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
    fn a_regular_text_diff_has_no_non_textual_classification() {
        let file = parse_file_diff(SAMPLE_DIFF).unwrap();
        assert_eq!(file.non_textual, None);
    }

    #[test]
    fn a_mode_only_change_is_classified_without_a_content_diff() {
        let text = "diff --git a/script.sh b/script.sh\nold mode 100644\nnew mode 100755\n";
        let file = parse_file_diff(text).unwrap();
        assert!(file.hunks.is_empty());
        assert_eq!(
            file.non_textual,
            Some(NonTextualDiff::ModeOnly {
                old_mode: "100644".to_string(),
                new_mode: "100755".to_string(),
            })
        );
    }

    #[test]
    fn a_submodule_gitlink_change_is_classified_with_both_commits() {
        let text = "diff --git a/sub b/sub\nindex 51f7df4..4ae7d63 160000\n--- a/sub\n+++ b/sub\n@@ -1 +1 @@\n-Subproject commit 51f7df4a1e256eeb1e6c7756e4b6247fa9b4f0f5\n+Subproject commit 4ae7d63b32339e560a6434a35fe9b2c162d4cedd\n";
        let file = parse_file_diff(text).unwrap();
        assert_eq!(
            file.non_textual,
            Some(NonTextualDiff::Submodule {
                old_commit: Some("51f7df4a1e256eeb1e6c7756e4b6247fa9b4f0f5".to_string()),
                new_commit: Some("4ae7d63b32339e560a6434a35fe9b2c162d4cedd".to_string()),
            })
        );
    }

    #[test]
    fn a_symlink_change_is_classified_with_both_targets() {
        let text = "diff --git a/link.txt b/link.txt\nindex f79a655..002d2d5 120000\n--- a/link.txt\n+++ b/link.txt\n@@ -1 +1 @@\n-target-a\n\\ No newline at end of file\n+target-b\n\\ No newline at end of file\n";
        let file = parse_file_diff(text).unwrap();
        assert_eq!(
            file.non_textual,
            Some(NonTextualDiff::Symlink {
                old_target: Some("target-a".to_string()),
                new_target: Some("target-b".to_string()),
            })
        );
    }

    #[test]
    fn a_binary_file_change_is_classified_with_sha_placeholders_for_size_lookup() {
        let text = "diff --git a/blob.bin b/blob.bin\nindex bdfd9fd..dbda5e5 100644\nBinary files a/blob.bin and b/blob.bin differ\n";
        let file = parse_file_diff(text).unwrap();
        assert!(file.is_binary);
        assert_eq!(
            file.non_textual,
            Some(NonTextualDiff::Binary {
                old_size: None,
                new_size: None,
                old_sha: Some("bdfd9fd".to_string()),
                new_sha: Some("dbda5e5".to_string()),
            })
        );
    }

    #[test]
    fn a_new_binary_file_reports_no_old_sha() {
        let text = "diff --git c/big.bin i/big.bin\nnew file mode 100644\nindex 0000000..0a622c8\nBinary files /dev/null and i/big.bin differ\n";
        let file = parse_file_diff(text).unwrap();
        match file.non_textual {
            Some(NonTextualDiff::Binary {
                old_sha, new_sha, ..
            }) => {
                assert_eq!(old_sha, None);
                assert_eq!(new_sha, Some("0a622c8".to_string()));
            }
            other => panic!("expected Binary, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn binary_blob_sizes_are_filled_in_via_a_real_cat_file_call() {
        let dir = init_repo();
        std::fs::write(dir.path().join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
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
        std::fs::write(dir.path().join("blob.bin"), [0u8, 1, 2, 3, 4, 5]).unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let file = diff_file(&layer, dir.path(), "blob.bin", false)
            .await
            .unwrap()
            .unwrap();
        match file.non_textual {
            Some(NonTextualDiff::Binary {
                old_size, new_size, ..
            }) => {
                assert_eq!(old_size, Some(4));
                assert_eq!(new_size, Some(6));
            }
            other => panic!("expected Binary, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_staged_binary_change_reads_both_sizes_via_cat_file() {
        let dir = init_repo();
        std::fs::write(dir.path().join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
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
        std::fs::write(dir.path().join("blob.bin"), [0u8, 1, 2, 3, 4, 5, 6]).unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .status()
            .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let file = diff_file(&layer, dir.path(), "blob.bin", true)
            .await
            .unwrap()
            .unwrap();
        match file.non_textual {
            Some(NonTextualDiff::Binary {
                old_size, new_size, ..
            }) => {
                assert_eq!(old_size, Some(4));
                assert_eq!(new_size, Some(7));
            }
            other => panic!("expected Binary, got {other:?}"),
        }
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

    #[tokio::test]
    async fn ignore_whitespace_view_option_hides_a_whitespace_only_change() {
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
        let options = DiffViewOptions {
            context_lines: None,
            ignore_whitespace: true,
        };
        let file = diff_file_with_options(&layer, dir.path(), "f.txt", false, &options)
            .await
            .unwrap();
        assert!(
            file.is_none(),
            "a whitespace-only change should vanish under --ignore-all-space"
        );

        // default view (no options) must still see the whitespace change
        let default_file = diff_file(&layer, dir.path(), "f.txt", false)
            .await
            .unwrap()
            .unwrap();
        assert!(default_file.hunks[0].lines.iter().any(|l| l == "+line2  "));
    }

    #[tokio::test]
    async fn a_large_context_lines_value_expands_to_the_whole_file() {
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
        lines[9] = "line10 changed".to_string();
        std::fs::write(dir.path().join("f.txt"), lines.join("\n") + "\n").unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let options = DiffViewOptions {
            context_lines: Some(10_000),
            ignore_whitespace: false,
        };
        let file = diff_file_with_options(&layer, dir.path(), "f.txt", false, &options)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            file.hunks.len(),
            1,
            "a huge context collapses to a single whole-file hunk"
        );
        assert!(file.hunks[0].lines.iter().any(|l| l == " line1"));
        assert!(file.hunks[0].lines.iter().any(|l| l == " line20"));
        assert!(file.hunks[0].lines.iter().any(|l| l == "-line10"));
        assert!(file.hunks[0].lines.iter().any(|l| l == "+line10 changed"));
    }
}
