use std::path::{Path, PathBuf};
use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

use crate::staging::stage_paths;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    BothModified,
    BothAdded,
    BothDeleted,
    DeleteModify,
    RenameRename,
    Submodule,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SubmoduleCandidateCommit {
    pub sha: String,
    pub short_sha: String,
    pub author: String,
    pub date: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SubmoduleConflictInfo {
    pub path: String,
    pub ours_commit: Option<SubmoduleCandidateCommit>,
    pub theirs_commit: Option<SubmoduleCandidateCommit>,
    pub base_commit: Option<SubmoduleCandidateCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConflictMarkerInfo {
    pub has_markers: bool,
    pub marker_count: usize,
    pub marker_lines: Vec<usize>,
    pub preview_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConflictItem {
    pub path: String,
    pub conflict_type: ConflictType,
    pub conflict_code: String,
    pub description: String,
    pub is_submodule: bool,
    pub submodule_info: Option<SubmoduleConflictInfo>,
    pub marker_info: Option<ConflictMarkerInfo>,
    pub ours_exists: bool,
    pub theirs_exists: bool,
    pub base_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "resolution_type", rename_all = "snake_case")]
pub enum ConflictResolution {
    Ours,
    Theirs,
    SubmoduleCommit { sha: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MergetoolConfig {
    pub configured_tool: Option<String>,
    pub available_tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MergetoolOutcome {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StageOutcome {
    Success,
    MarkerRefusal {
        file: String,
        marker_lines: Vec<usize>,
        preview_lines: Vec<String>,
    },
}

pub fn check_conflict_markers_in_text(content: &str) -> ConflictMarkerInfo {
    let mut marker_lines = Vec::new();
    let mut preview_lines = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let is_marker = line.starts_with("<<<<<<<")
            || (line.starts_with("=======")
                && (line.trim() == "=======" || line.trim_start_matches('=').is_empty()))
            || line.starts_with(">>>>>>>")
            || line.starts_with("|||||||");

        if is_marker {
            marker_lines.push(line_num);
            let truncated = if line.chars().count() > 80 {
                format!("{}...", line.chars().take(77).collect::<String>())
            } else {
                line.to_string()
            };
            preview_lines.push(format!("Line {}: {}", line_num, truncated));
        }
    }

    ConflictMarkerInfo {
        has_markers: !marker_lines.is_empty(),
        marker_count: marker_lines.len(),
        marker_lines,
        preview_lines,
    }
}

pub fn check_file_conflict_markers(root: &Path, rel_path: &Path) -> Option<ConflictMarkerInfo> {
    let full = root.join(rel_path);
    if !full.is_file() {
        return None;
    }
    let data = std::fs::read(&full).ok()?;
    let content = match std::str::from_utf8(&data) {
        Ok(s) => s,
        Err(_) => return None,
    };
    Some(check_conflict_markers_in_text(content))
}

pub async fn query_submodule_commit(
    layer: &ProcessLayer,
    root: &Path,
    submodule_path: &str,
    sha: &str,
) -> Option<SubmoduleCandidateCommit> {
    if sha.is_empty() || sha.chars().all(|c| c == '0') {
        return None;
    }

    let sub_work = root.join(submodule_path);
    let target_dir = if sub_work.exists() {
        &sub_work
    } else {
        root
    };

    let res = layer
        .run(
            GitCall::new(
                target_dir,
                ["log", "-1", "--format=%H%x00%h%x00%an%x00%ad%x00%s", sha],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .ok()?;

    if res.ok() {
        let text = res.stdout_utf8_lossy();
        let parts: Vec<&str> = text.trim_end_matches('\n').split('\0').collect();
        if parts.len() >= 5 {
            return Some(SubmoduleCandidateCommit {
                sha: parts[0].to_string(),
                short_sha: parts[1].to_string(),
                author: parts[2].to_string(),
                date: parts[3].to_string(),
                subject: parts[4].to_string(),
            });
        }
    }

    Some(SubmoduleCandidateCommit {
        sha: sha.to_string(),
        short_sha: if sha.len() >= 7 {
            sha[..7].to_string()
        } else {
            sha.to_string()
        },
        author: String::new(),
        date: String::new(),
        subject: "(Submodule commit details unavailable locally)".to_string(),
    })
}

pub async fn query_conflicts(
    layer: &ProcessLayer,
    root: &Path,
) -> Result<Vec<ConflictItem>, String> {
    let res = layer
        .run(
            GitCall::new(
                root,
                ["status", "--porcelain=v2", "-z", "--untracked-files=all"],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error querying status: {e}"))?;

    if !res.ok() {
        return Err(format!("git status failed: {}", res.stderr.trim()));
    }

    let text = String::from_utf8_lossy(&res.stdout);
    let mut items = Vec::new();

    for entry in text.split('\0') {
        if entry.is_empty() || !entry.starts_with("u ") {
            continue;
        }

        let rest = &entry[2..];
        let f: Vec<&str> = rest.splitn(10, ' ').collect();
        if f.len() < 10 {
            continue;
        }

        let code = f[0];
        let sub = f[1];
        let m1 = f[2];
        let m2 = f[3];
        let m3 = f[4];
        let h1 = f[6];
        let h2 = f[7];
        let h3 = f[8];
        let path = f[9];

        let base_exists = m1 != "000000" && !h1.chars().all(|c| c == '0');
        let ours_exists = m2 != "000000" && !h2.chars().all(|c| c == '0');
        let theirs_exists = m3 != "000000" && !h3.chars().all(|c| c == '0');

        let is_submodule = sub.starts_with('S') || m1 == "160000" || m2 == "160000" || m3 == "160000";

        let conflict_type = if is_submodule {
            ConflictType::Submodule
        } else {
            match code {
                "UU" => ConflictType::BothModified,
                "AA" => ConflictType::BothAdded,
                "DD" => ConflictType::BothDeleted,
                "UD" | "DU" => ConflictType::DeleteModify,
                "UA" | "AU" => ConflictType::RenameRename,
                other => ConflictType::Other(other.to_string()),
            }
        };

        let description = match &conflict_type {
            ConflictType::BothModified => "Both modified by us and them".to_string(),
            ConflictType::BothAdded => "Both added independently".to_string(),
            ConflictType::BothDeleted => "Both deleted by us and them".to_string(),
            ConflictType::DeleteModify => {
                if code == "UD" {
                    "Modified by us, deleted by them".to_string()
                } else {
                    "Deleted by us, modified by them".to_string()
                }
            }
            ConflictType::RenameRename => {
                if code == "UA" {
                    "Rename conflict (destination added by them)".to_string()
                } else if code == "AU" {
                    "Rename conflict (destination added by us)".to_string()
                } else {
                    "Rename conflict".to_string()
                }
            }
            ConflictType::Submodule => "Submodule gitlink conflict".to_string(),
            ConflictType::Other(c) => format!("Unmerged conflict ({c})"),
        };

        let submodule_info = if is_submodule {
            let ours_commit = query_submodule_commit(layer, root, path, h2).await;
            let theirs_commit = query_submodule_commit(layer, root, path, h3).await;
            let base_commit = query_submodule_commit(layer, root, path, h1).await;
            Some(SubmoduleConflictInfo {
                path: path.to_string(),
                ours_commit,
                theirs_commit,
                base_commit,
            })
        } else {
            None
        };

        let marker_info = if !is_submodule {
            check_file_conflict_markers(root, Path::new(path))
        } else {
            None
        };

        items.push(ConflictItem {
            path: path.to_string(),
            conflict_type,
            conflict_code: code.to_string(),
            description,
            is_submodule,
            submodule_info,
            marker_info,
            ours_exists,
            theirs_exists,
            base_exists,
        });
    }

    items.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(items)
}

pub async fn stage_paths_with_guard(
    layer: &ProcessLayer,
    root: &Path,
    paths: &[PathBuf],
    override_markers: bool,
) -> Result<StageOutcome, String> {
    if !override_markers {
        for p in paths {
            if let Some(info) = check_file_conflict_markers(root, p) {
                if info.has_markers {
                    return Ok(StageOutcome::MarkerRefusal {
                        file: p.to_string_lossy().to_string(),
                        marker_lines: info.marker_lines,
                        preview_lines: info.preview_lines,
                    });
                }
            }
        }
    }

    stage_paths(layer, root, paths).await?;
    Ok(StageOutcome::Success)
}

pub async fn resolve_conflict(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    resolution: ConflictResolution,
) -> Result<(), String> {
    let conflicts = query_conflicts(layer, root).await.unwrap_or_default();
    let current_item = conflicts.iter().find(|i| i.path == path);

    match resolution {
        ConflictResolution::Ours => {
            if let Some(item) = current_item {
                if item.is_submodule {
                    let res = layer
                        .run(
                            GitCall::new(root, ["checkout", "--ours", "--", path]),
                            Intent::Write,
                            CancellationToken::new(),
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                    if !res.ok() {
                        return Err(format!("git checkout --ours failed: {}", res.stderr.trim()));
                    }
                    if let Some(sub_info) = &item.submodule_info {
                        if let Some(ours_c) = &sub_info.ours_commit {
                            let sub_dir = root.join(path);
                            if sub_dir.exists() {
                                let _ = layer
                                    .run(
                                        GitCall::new(&sub_dir, ["checkout", &ours_c.sha]),
                                        Intent::Write,
                                        CancellationToken::new(),
                                    )
                                    .await;
                            }
                        }
                    }
                    stage_paths(layer, root, &[PathBuf::from(path)]).await?;
                    return Ok(());
                }

                if item.conflict_code == "DU" || item.conflict_code == "UA" {
                    let res = layer
                        .run(
                            GitCall::new(root, ["rm", "-f", "--", path]),
                            Intent::Write,
                            CancellationToken::new(),
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                    if !res.ok() {
                        return Err(format!("git rm failed: {}", res.stderr.trim()));
                    }
                    return Ok(());
                }
            }

            let res = layer
                .run(
                    GitCall::new(root, ["checkout", "--ours", "--", path]),
                    Intent::Write,
                    CancellationToken::new(),
                )
                .await
                .map_err(|e| e.to_string())?;
            if !res.ok() {
                return Err(format!("git checkout --ours failed: {}", res.stderr.trim()));
            }
            stage_paths(layer, root, &[PathBuf::from(path)]).await?;
            Ok(())
        }
        ConflictResolution::Theirs => {
            if let Some(item) = current_item {
                if item.is_submodule {
                    let res = layer
                        .run(
                            GitCall::new(root, ["checkout", "--theirs", "--", path]),
                            Intent::Write,
                            CancellationToken::new(),
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                    if !res.ok() {
                        return Err(format!("git checkout --theirs failed: {}", res.stderr.trim()));
                    }
                    if let Some(sub_info) = &item.submodule_info {
                        if let Some(theirs_c) = &sub_info.theirs_commit {
                            let sub_dir = root.join(path);
                            if sub_dir.exists() {
                                let _ = layer
                                    .run(
                                        GitCall::new(&sub_dir, ["checkout", &theirs_c.sha]),
                                        Intent::Write,
                                        CancellationToken::new(),
                                    )
                                    .await;
                            }
                        }
                    }
                    stage_paths(layer, root, &[PathBuf::from(path)]).await?;
                    return Ok(());
                }

                if item.conflict_code == "UD" || item.conflict_code == "AU" {
                    let res = layer
                        .run(
                            GitCall::new(root, ["rm", "-f", "--", path]),
                            Intent::Write,
                            CancellationToken::new(),
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                    if !res.ok() {
                        return Err(format!("git rm failed: {}", res.stderr.trim()));
                    }
                    return Ok(());
                }
            }

            let res = layer
                .run(
                    GitCall::new(root, ["checkout", "--theirs", "--", path]),
                    Intent::Write,
                    CancellationToken::new(),
                )
                .await
                .map_err(|e| e.to_string())?;
            if !res.ok() {
                return Err(format!("git checkout --theirs failed: {}", res.stderr.trim()));
            }
            stage_paths(layer, root, &[PathBuf::from(path)]).await?;
            Ok(())
        }
        ConflictResolution::SubmoduleCommit { sha } => {
            let sub_dir = root.join(path);
            if sub_dir.exists() {
                let res = layer
                    .run(
                        GitCall::new(&sub_dir, ["checkout", &sha]),
                        Intent::Write,
                        CancellationToken::new(),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                if !res.ok() {
                    return Err(format!(
                        "git checkout in submodule failed: {}",
                        res.stderr.trim()
                    ));
                }
            }
            stage_paths(layer, root, &[PathBuf::from(path)]).await?;
            Ok(())
        }
    }
}

pub async fn launch_mergetool(
    layer: &ProcessLayer,
    root: &Path,
    path: &str,
    tool: Option<String>,
) -> Result<MergetoolOutcome, String> {
    let mut args = vec!["mergetool".to_string(), "-y".to_string()];
    if let Some(t) = tool {
        args.push(format!("--tool={t}"));
    }
    args.push("--".to_string());
    args.push(path.to_string());

    let res = layer
        .run(
            GitCall::new(root, args.iter().map(|s| s.as_str())),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error running git mergetool: {e}"))?;

    Ok(MergetoolOutcome {
        success: res.ok(),
        exit_code: res.status,
        stdout: res.stdout_utf8_lossy().to_string(),
        stderr: res.stderr,
    })
}

pub async fn query_mergetool_config(
    layer: &ProcessLayer,
    root: &Path,
) -> Result<MergetoolConfig, String> {
    let res_cfg = layer
        .run(
            GitCall::new(root, ["config", "--get", "merge.tool"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Failed to read git config: {e}"))?;

    let configured_tool = if res_cfg.ok() {
        let val = res_cfg.stdout_utf8_lossy().trim().to_string();
        if val.is_empty() {
            None
        } else {
            Some(val)
        }
    } else {
        None
    };

    let res_tools = layer
        .run(
            GitCall::new(root, ["mergetool", "--tool-help"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Failed to run git mergetool --tool-help: {e}"))?;

    let mut available_tools = Vec::new();
    let stdout = res_tools.stdout_utf8_lossy();
    let mut in_available_section = false;

    for line in stdout.lines() {
        if line.contains("may be set to one of the following:") {
            in_available_section = true;
            continue;
        }
        if line.contains("The following tools are valid, but not currently available:") {
            break;
        }
        if in_available_section {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Some(tool_name) = trimmed.split_whitespace().next() {
                    available_tools.push(tool_name.to_string());
                }
            }
        }
    }

    Ok(MergetoolConfig {
        configured_tool,
        available_tools,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_repo(root: &Path) {
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(root)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(root)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(root)
            .output()
            .unwrap();
    }

    #[test]
    fn test_conflict_marker_detection_and_line_numbers() {
        let sample = "clean line 1\nclean line 2\n<<<<<<< HEAD\nline from ours\n=======\nline from theirs\n>>>>>>> feature-branch\nclean line 8\n";
        let info = check_conflict_markers_in_text(sample);
        assert!(info.has_markers);
        assert_eq!(info.marker_count, 3);
        assert_eq!(info.marker_lines, vec![3, 5, 7]);
        assert_eq!(info.preview_lines[0], "Line 3: <<<<<<< HEAD");
        assert_eq!(info.preview_lines[1], "Line 5: =======");
        assert_eq!(info.preview_lines[2], "Line 7: >>>>>>> feature-branch");

        let clean = "clean line 1\nclean line 2\nclean line 3\n";
        let clean_info = check_conflict_markers_in_text(clean);
        assert!(!clean_info.has_markers);
        assert_eq!(clean_info.marker_count, 0);
        assert!(clean_info.marker_lines.is_empty());
    }

    #[tokio::test]
    async fn test_stage_with_conflict_marker_guard_refusal_and_override() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        setup_repo(root);

        let file = root.join("conflict.txt");
        std::fs::write(&file, "first line\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\nlast line\n").unwrap();

        let layer = ProcessLayer::new(4, std::time::Duration::from_secs(10));

        // 1. Stage without override -> MarkerRefusal naming the marker lines
        let outcome = stage_paths_with_guard(&layer, root, &[PathBuf::from("conflict.txt")], false)
            .await
            .unwrap();

        match outcome {
            StageOutcome::MarkerRefusal { file, marker_lines, preview_lines } => {
                assert_eq!(file, "conflict.txt");
                assert_eq!(marker_lines, vec![2, 4, 6]);
                assert_eq!(preview_lines.len(), 3);
                assert!(preview_lines[0].contains("Line 2: <<<<<<< HEAD"));
            }
            StageOutcome::Success => panic!("Expected marker refusal!"),
        }

        // 2. Stage with override -> Success!
        let outcome2 = stage_paths_with_guard(&layer, root, &[PathBuf::from("conflict.txt")], true)
            .await
            .unwrap();
        assert_eq!(outcome2, StageOutcome::Success);
    }

    #[tokio::test]
    async fn test_conflict_classification_both_modified_and_added() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        setup_repo(root);

        // Commit base
        std::fs::write(root.join("f1.txt"), "base 1\n").unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(root).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "base"]).current_dir(root).output().unwrap();

        // Branch 1: modify f1, add f2
        std::process::Command::new("git").args(["checkout", "-b", "b1"]).current_dir(root).output().unwrap();
        std::fs::write(root.join("f1.txt"), "b1 1\n").unwrap();
        std::fs::write(root.join("f2.txt"), "b1 2\n").unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(root).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "b1"]).current_dir(root).output().unwrap();

        // Branch 2 from main: modify f1 differently, add f2 differently
        std::process::Command::new("git").args(["checkout", "main"]).current_dir(root).output().unwrap();
        std::process::Command::new("git").args(["checkout", "-b", "b2"]).current_dir(root).output().unwrap();
        std::fs::write(root.join("f1.txt"), "b2 1\n").unwrap();
        std::fs::write(root.join("f2.txt"), "b2 2\n").unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(root).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "b2"]).current_dir(root).output().unwrap();

        // Merge b1 into b2 -> conflicts in f1 (both modified) and f2 (both added)
        std::process::Command::new("git").args(["merge", "b1"]).current_dir(root).output().unwrap();

        let layer = ProcessLayer::new(4, std::time::Duration::from_secs(10));
        let conflicts = query_conflicts(&layer, root).await.unwrap();
        assert_eq!(conflicts.len(), 2);

        let f1 = conflicts.iter().find(|c| c.path == "f1.txt").unwrap();
        assert_eq!(f1.conflict_type, ConflictType::BothModified);
        assert_eq!(f1.conflict_code, "UU");
        assert!(!f1.is_submodule);
        assert!(f1.marker_info.as_ref().unwrap().has_markers);

        let f2 = conflicts.iter().find(|c| c.path == "f2.txt").unwrap();
        assert_eq!(f2.conflict_type, ConflictType::BothAdded);
        assert_eq!(f2.conflict_code, "AA");

        // Resolve f1 using ours
        resolve_conflict(&layer, root, "f1.txt", ConflictResolution::Ours).await.unwrap();
        let content1 = std::fs::read_to_string(root.join("f1.txt")).unwrap();
        assert_eq!(content1.trim(), "b2 1");

        // Resolve f2 using theirs
        resolve_conflict(&layer, root, "f2.txt", ConflictResolution::Theirs).await.unwrap();
        let content2 = std::fs::read_to_string(root.join("f2.txt")).unwrap();
        assert_eq!(content2.trim(), "b1 2");

        // Post-resolution conflicts must be empty
        let remaining = query_conflicts(&layer, root).await.unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_delete_modify_conflict_classification_and_resolution() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        setup_repo(root);

        std::fs::write(root.join("file.txt"), "base\n").unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(root).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "base"]).current_dir(root).output().unwrap();

        // Branch b_delete deletes file
        std::process::Command::new("git").args(["checkout", "-b", "b_delete"]).current_dir(root).output().unwrap();
        std::process::Command::new("git").args(["rm", "file.txt"]).current_dir(root).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "delete"]).current_dir(root).output().unwrap();

        // Branch b_mod modifies file
        std::process::Command::new("git").args(["checkout", "main"]).current_dir(root).output().unwrap();
        std::process::Command::new("git").args(["checkout", "-b", "b_mod"]).current_dir(root).output().unwrap();
        std::fs::write(root.join("file.txt"), "mod content\n").unwrap();
        std::process::Command::new("git").args(["commit", "-am", "mod"]).current_dir(root).output().unwrap();

        // Merge b_delete into b_mod -> UD conflict (ours modified, theirs deleted)
        std::process::Command::new("git").args(["merge", "b_delete"]).current_dir(root).output().unwrap();

        let layer = ProcessLayer::new(4, std::time::Duration::from_secs(10));
        let conflicts = query_conflicts(&layer, root).await.unwrap();
        assert_eq!(conflicts.len(), 1);
        let c = &conflicts[0];
        assert_eq!(c.conflict_type, ConflictType::DeleteModify);
        assert_eq!(c.conflict_code, "UD");
        assert!(c.ours_exists);
        assert!(!c.theirs_exists);

        // Resolve using theirs (accept deletion)
        resolve_conflict(&layer, root, "file.txt", ConflictResolution::Theirs).await.unwrap();
        assert!(!root.join("file.txt").exists());

        let remaining = query_conflicts(&layer, root).await.unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_submodule_gitlink_conflict_surfacing_and_candidate_commits() {
        let dir = tempdir().unwrap();
        let sub_dir = dir.path().join("sub_repo");
        let super_dir = dir.path().join("super_repo");
        std::fs::create_dir_all(&sub_dir).unwrap();
        std::fs::create_dir_all(&super_dir).unwrap();

        setup_repo(&sub_dir);
        std::fs::write(sub_dir.join("README"), "init\n").unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&sub_dir).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "sub initial"]).current_dir(&sub_dir).output().unwrap();

        setup_repo(&super_dir);
        std::fs::write(super_dir.join("root.txt"), "root\n").unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&super_dir).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "super initial"]).current_dir(&super_dir).output().unwrap();

        // Add submodule
        std::process::Command::new("git")
            .args(["-c", "protocol.file.allow=always", "submodule", "add", sub_dir.to_str().unwrap(), "mysub"])
            .current_dir(&super_dir)
            .output()
            .unwrap();
        std::process::Command::new("git").args(["commit", "-m", "add submodule"]).current_dir(&super_dir).output().unwrap();

        // Branch 1: advance submodule to commit A
        std::process::Command::new("git").args(["checkout", "-b", "branch1"]).current_dir(&super_dir).output().unwrap();
        let sub_work = super_dir.join("mysub");
        std::fs::write(sub_work.join("README"), "commit A\n").unwrap();
        std::process::Command::new("git").args(["commit", "-am", "sub commit A (ours)"]).current_dir(&sub_work).output().unwrap();
        std::process::Command::new("git").args(["add", "mysub"]).current_dir(&super_dir).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "super branch1"]).current_dir(&super_dir).output().unwrap();

        // Branch 2 from main: advance submodule to commit B
        std::process::Command::new("git").args(["checkout", "main"]).current_dir(&super_dir).output().unwrap();
        std::process::Command::new("git").args(["checkout", "-b", "branch2"]).current_dir(&super_dir).output().unwrap();
        std::process::Command::new("git").args(["submodule", "update", "--checkout"]).current_dir(&super_dir).output().unwrap();
        std::fs::write(sub_work.join("README"), "commit B\n").unwrap();
        std::process::Command::new("git").args(["commit", "-am", "sub commit B (theirs)"]).current_dir(&sub_work).output().unwrap();
        std::process::Command::new("git").args(["add", "mysub"]).current_dir(&super_dir).output().unwrap();
        std::process::Command::new("git").args(["commit", "-m", "super branch2"]).current_dir(&super_dir).output().unwrap();

        // Merge branch1 into branch2 -> submodule conflict!
        std::process::Command::new("git").args(["merge", "branch1"]).current_dir(&super_dir).output().unwrap();

        let layer = ProcessLayer::new(4, std::time::Duration::from_secs(10));
        let conflicts = query_conflicts(&layer, &super_dir).await.unwrap();
        assert_eq!(conflicts.len(), 1);

        let sub_conflict = &conflicts[0];
        assert_eq!(sub_conflict.path, "mysub");
        assert_eq!(sub_conflict.conflict_type, ConflictType::Submodule);
        assert!(sub_conflict.is_submodule);

        let sub_info = sub_conflict.submodule_info.as_ref().expect("Expected submodule info");
        let ours_commit = sub_info.ours_commit.as_ref().expect("Expected ours commit");
        let theirs_commit = sub_info.theirs_commit.as_ref().expect("Expected theirs commit");

        // Candidates must show commit subject
        assert_eq!(ours_commit.subject, "sub commit B (theirs)");
        assert_eq!(theirs_commit.subject, "sub commit A (ours)");

        // Resolve by selecting candidate commit A
        resolve_conflict(&layer, &super_dir, "mysub", ConflictResolution::SubmoduleCommit { sha: theirs_commit.sha.clone() })
            .await
            .unwrap();

        let remaining = query_conflicts(&layer, &super_dir).await.unwrap();
        assert!(remaining.is_empty());
    }
}
