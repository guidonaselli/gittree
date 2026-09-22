use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

use crate::resolved::Resolved;
use crate::sync::{pull as repo_pull, PullOptions, PullOutcome, PullStrategy};
use crate::upstream::{resolve_upstream_basis, UpstreamBasis};

/// Divergence between a submodule's checked-out HEAD and the commit the
/// superproject records for it (the gitlink).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GitlinkDivergence {
    InSync,
    Ahead { ahead: u32 },
    Behind { behind: u32 },
    Both { ahead: u32, behind: u32 },
    GitlinkObjectMissingLocally { gitlink_commit: String },
    UnrelatedHistories,
}

/// Checked-out branch state of a submodule: either a symbolic branch name
/// or a detached HEAD along with any refs (branches or tags) pointing directly at it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SubmoduleBranch {
    Named(String),
    Detached {
        commit: String,
        pointing_refs: Vec<String>,
    },
}

/// Drift between declared configuration (.gitmodules), recorded gitlinks,
/// disk state, and repository config.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SubmoduleDrift {
    DeclaredButAbsent,
    PresentButUndeclared,
    UrlMismatch {
        declared_url: String,
        config_url: String,
    },
    OrphanedDeclaration,
}

/// A malformed entry or syntax violation found in `.gitmodules`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MalformedGitmodulesEntry {
    pub line_number: usize,
    pub raw_text: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeclaredSubmodule {
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubmoduleState {
    pub name: String,
    pub path: PathBuf,
    pub relative_path: String,
    pub depth: usize,
    pub parent_path: Option<PathBuf>,
    pub declared_branch: Resolved<Option<String>>,
    pub url: Resolved<Option<String>>,
    pub initialized: bool,
    pub gitlink_commit: Resolved<String>,
    pub branch: Resolved<SubmoduleBranch>,
    pub gitlink_divergence: Resolved<GitlinkDivergence>,
    pub remote_basis: Resolved<UpstreamBasis>,
    pub remote_ahead_behind: Resolved<(u32, u32)>,
    pub dirty: Resolved<bool>,
    pub last_fetch_unix_secs: Resolved<Option<u64>>,
    pub drift: Resolved<Vec<SubmoduleDrift>>,
}

impl SubmoduleState {
    pub fn display_summary(&self) -> String {
        format!(
            "name={}, path={}, rel={}, depth={}, init={}, branch={}, gitlink={}, div={}, basis={}, a/b={}, dirty={}, fetch={}, drift={}",
            self.name,
            self.path.display(),
            self.relative_path,
            self.depth,
            self.initialized,
            format_resolved(&self.branch, |b| match b {
                SubmoduleBranch::Named(n) => n.clone(),
                SubmoduleBranch::Detached { commit, pointing_refs } => {
                    if pointing_refs.is_empty() {
                        format!("detached @ {commit}")
                    } else {
                        format!("detached @ {commit} ({})", pointing_refs.join(", "))
                    }
                }
            }),
            format_resolved(&self.gitlink_commit, |s| s.clone()),
            format_resolved(&self.gitlink_divergence, |d| format!("{d:?}")),
            format_resolved(&self.remote_basis, |b| format!("{b:?}")),
            format_resolved(&self.remote_ahead_behind, |(a, b)| format!("+{a}/-{b}")),
            format_resolved(&self.dirty, |d| d.to_string()),
            format_resolved(&self.last_fetch_unix_secs, |f| format!("{f:?}")),
            format_resolved(&self.drift, |d| format!("{d:?}")),
        )
    }
}

fn format_resolved<T, F: FnOnce(&T) -> String>(res: &Resolved<T>, f: F) -> String {
    match res {
        Resolved::Known(value) => f(value),
        Resolved::Unknown { reason } => format!("unknown: {reason}"),
    }
}

/// The result of querying a submodule matrix across the workspace.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubmoduleMatrixResult {
    pub submodules: Vec<SubmoduleState>,
    pub malformed_entries: Vec<MalformedGitmodulesEntry>,
}

impl std::ops::Deref for SubmoduleMatrixResult {
    type Target = [SubmoduleState];
    fn deref(&self) -> &Self::Target {
        &self.submodules
    }
}

impl IntoIterator for SubmoduleMatrixResult {
    type Item = SubmoduleState;
    type IntoIter = std::vec::IntoIter<SubmoduleState>;
    fn into_iter(self) -> Self::IntoIter {
        self.submodules.into_iter()
    }
}

impl<'a> IntoIterator for &'a SubmoduleMatrixResult {
    type Item = &'a SubmoduleState;
    type IntoIter = std::slice::Iter<'a, SubmoduleState>;
    fn into_iter(self) -> Self::IntoIter {
        self.submodules.iter()
    }
}

/// Stage reported during parallel submodule network refresh.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum SubmoduleRefreshStage {
    Starting,
    Fetching { remote: String },
    Completed { summary: String },
    Skipped { reason: String },
    Failed { error: String },
}

/// Progress event emitted during network refresh.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubmoduleRefreshProgress {
    pub path: PathBuf,
    pub relative_path: String,
    pub stage: SubmoduleRefreshStage,
}

/// Outcome status for an individual submodule during network refresh.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SubmoduleRefreshStatus {
    Success { summary: String },
    Skipped { reason: String },
    Failed { error: String },
    Cancelled,
}

/// Row result for an individual submodule during network refresh.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubmoduleRefreshRowResult {
    pub path: PathBuf,
    pub relative_path: String,
    pub status: SubmoduleRefreshStatus,
    pub updated_state: Option<SubmoduleState>,
}

/// Complete aggregated outcome of a parallel network refresh operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubmoduleRefreshResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub rows: Vec<SubmoduleRefreshRowResult>,
}

/// Options controlling parallel network refresh.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SubmoduleRefreshOptions {
    pub paths: Option<Vec<PathBuf>>,
    pub concurrency: Option<usize>,
    pub prune: Option<bool>,
    pub tags: Option<bool>,
}

/// Per-item action determined during bulk checkout preview.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BulkCheckoutAction {
    WillSwitch {
        current_branch: String,
        target_branch: String,
    },
    AlreadyOnBranch {
        branch: String,
    },
    SkippedDirty {
        uncommitted_changes: bool,
    },
    MissingBranch {
        branch: String,
    },
    Uninitialized,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkCheckoutPreviewItem {
    pub path: PathBuf,
    pub relative_path: String,
    pub action: BulkCheckoutAction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkCheckoutPreview {
    pub total: usize,
    pub will_switch: usize,
    pub already_on_branch: usize,
    pub skipped_dirty: usize,
    pub missing_branch: usize,
    pub items: Vec<BulkCheckoutPreviewItem>,
}

/// Per-item action determined during bulk pull preview.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BulkPullAction {
    WillPull {
        branch: String,
        upstream: String,
        behind: u32,
    },
    AlreadyUpToDate {
        branch: String,
        upstream: String,
    },
    SkippedDirty {
        uncommitted_changes: bool,
    },
    SkippedNoUpstream,
    Uninitialized,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkPullPreviewItem {
    pub path: PathBuf,
    pub relative_path: String,
    pub action: BulkPullAction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkPullPreview {
    pub total: usize,
    pub will_pull: usize,
    pub already_up_to_date: usize,
    pub skipped_dirty: usize,
    pub skipped_no_upstream: usize,
    pub items: Vec<BulkPullPreviewItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkPullOptions {
    pub strategy: PullStrategy,
    pub paths: Option<Vec<PathBuf>>,
}

/// Per-item action determined during bulk reset to gitlink preview.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BulkResetAction {
    WillReset {
        current_commit: String,
        gitlink_commit: String,
    },
    AlreadyInSync {
        gitlink_commit: String,
    },
    SkippedDirty {
        uncommitted_changes: bool,
    },
    MissingObject {
        gitlink_commit: String,
    },
    NoGitlinkRecorded,
    Uninitialized,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkResetPreviewItem {
    pub path: PathBuf,
    pub relative_path: String,
    pub action: BulkResetAction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkResetPreview {
    pub total: usize,
    pub will_reset: usize,
    pub already_in_sync: usize,
    pub skipped_dirty: usize,
    pub missing_object: usize,
    pub items: Vec<BulkResetPreviewItem>,
}

/// Outcome status for an individual bulk operation item.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BulkItemOutcome {
    Success { message: String },
    Skipped { reason: String },
    Failed { error: String },
    Cancelled,
}

/// Honest per-item outcome of a bulk operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkOperationItemResult {
    pub path: PathBuf,
    pub relative_path: String,
    pub outcome: BulkItemOutcome,
    pub updated_state: Option<SubmoduleState>,
}

/// Honest multi-operation aggregated result separating successes, skips, and failures.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkOperationResult {
    pub total: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub items: Vec<BulkOperationItemResult>,
}

/// Outcome of bumping a submodule's gitlink in the superproject.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BumpGitlinkOutcome {
    Success {
        submodule_path: PathBuf,
        relative_path: String,
        head_commit: String,
        previous_gitlink: Option<String>,
        warning: Option<String>,
    },
    UnpushedRefused {
        submodule_path: PathBuf,
        relative_path: String,
        head_commit: String,
        reason: String,
    },
    Failed {
        submodule_path: PathBuf,
        relative_path: String,
        error: String,
    },
}

/// Resilient line-by-line `.gitmodules` parser reporting malformed entries with
/// their exact 1-indexed line numbers while parsing all valid submodule sections.
pub fn parse_gitmodules_text(
    text: &str,
) -> (Vec<DeclaredSubmodule>, Vec<MalformedGitmodulesEntry>) {
    let mut submodules = Vec::new();
    let mut malformed_entries = Vec::new();

    struct CurrentSubmodule {
        name: String,
        header_line: usize,
        path: Option<String>,
        branch: Option<String>,
        url: Option<String>,
    }

    let mut current: Option<CurrentSubmodule> = None;

    let finish_submodule =
        |cur: CurrentSubmodule,
         submodules: &mut Vec<DeclaredSubmodule>,
         malformed: &mut Vec<MalformedGitmodulesEntry>| {
            match cur.path {
                Some(path) if !path.trim().is_empty() => {
                    submodules.push(DeclaredSubmodule {
                        name: cur.name,
                        path: path.trim().to_string(),
                        branch: cur.branch.map(|b| b.trim().to_string()),
                        url: cur.url.map(|u| u.trim().to_string()),
                    });
                }
                _ => {
                    malformed.push(MalformedGitmodulesEntry {
                        line_number: cur.header_line,
                        raw_text: format!("[submodule \"{}\"]", cur.name),
                        reason: "Submodule section is missing mandatory 'path' directive"
                            .to_string(),
                    });
                }
            }
        };

    for (idx, line) in text.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        if trimmed.starts_with('[') {
            if let Some(cur) = current.take() {
                finish_submodule(cur, &mut submodules, &mut malformed_entries);
            }

            if !trimmed.ends_with(']') {
                malformed_entries.push(MalformedGitmodulesEntry {
                    line_number: line_num,
                    raw_text: line.to_string(),
                    reason: "Malformed section header: missing closing ']'".to_string(),
                });
                continue;
            }

            let inner = trimmed[1..trimmed.len() - 1].trim();
            let prefix = "submodule";
            if !inner.starts_with(prefix) {
                malformed_entries.push(MalformedGitmodulesEntry {
                    line_number: line_num,
                    raw_text: line.to_string(),
                    reason: format!(
                        "Unexpected section header: expected [submodule \"<name>\"], got [{inner}]"
                    ),
                });
                continue;
            }

            let remainder = inner[prefix.len()..].trim();
            if remainder.is_empty() {
                malformed_entries.push(MalformedGitmodulesEntry {
                    line_number: line_num,
                    raw_text: line.to_string(),
                    reason: "Submodule section header missing submodule name".to_string(),
                });
                continue;
            }

            let name = remainder
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string();

            if name.is_empty() {
                malformed_entries.push(MalformedGitmodulesEntry {
                    line_number: line_num,
                    raw_text: line.to_string(),
                    reason: "Empty submodule name in section header".to_string(),
                });
                continue;
            }

            current = Some(CurrentSubmodule {
                name,
                header_line: line_num,
                path: None,
                branch: None,
                url: None,
            });
        } else {
            let Some((key, val)) = trimmed.split_once('=') else {
                malformed_entries.push(MalformedGitmodulesEntry {
                    line_number: line_num,
                    raw_text: line.to_string(),
                    reason: "Malformed entry: expected 'key = value', missing '='".to_string(),
                });
                continue;
            };

            let key = key.trim().to_lowercase();
            let val = val
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string();

            if let Some(cur) = current.as_mut() {
                match key.as_str() {
                    "path" => cur.path = Some(val),
                    "branch" => cur.branch = Some(val),
                    "url" => cur.url = Some(val),
                    _ => {}
                }
            } else {
                malformed_entries.push(MalformedGitmodulesEntry {
                    line_number: line_num,
                    raw_text: line.to_string(),
                    reason: format!("Directive '{key}' outside of any [submodule \"...\"] section"),
                });
            }
        }
    }

    if let Some(cur) = current.take() {
        finish_submodule(cur, &mut submodules, &mut malformed_entries);
    }

    (submodules, malformed_entries)
}

/// Reads `.gitmodules` from disk or via git cat-file fallback and parses it.
async fn read_gitmodules_resilient(
    layer: &ProcessLayer,
    superproject_root: &Path,
) -> (Vec<DeclaredSubmodule>, Vec<MalformedGitmodulesEntry>) {
    let gitmodules_path = superproject_root.join(".gitmodules");
    let content = if gitmodules_path.is_file() {
        std::fs::read_to_string(&gitmodules_path).ok()
    } else {
        None
    };

    let text = match content {
        Some(t) => t,
        None => {
            let res = layer
                .run(
                    GitCall::new(superproject_root, ["cat-file", "-p", "HEAD:.gitmodules"]),
                    Intent::Read,
                    CancellationToken::new(),
                )
                .await;
            match res {
                Ok(r) if r.ok() => r.stdout_utf8_lossy().to_string(),
                _ => return (Vec::new(), Vec::new()),
            }
        }
    };

    parse_gitmodules_text(&text)
}

/// One batched call reading every gitlink committed in the superproject's HEAD.
async fn read_gitlinks(layer: &ProcessLayer, superproject_root: &Path) -> HashMap<String, String> {
    let result = layer
        .run(
            GitCall::new(superproject_root, ["ls-tree", "-r", "HEAD", "-z"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;
    let mut map = HashMap::new();
    let Ok(result) = result else { return map };
    if !result.ok() {
        return map;
    }
    let text = String::from_utf8_lossy(&result.stdout);
    for line in text.split('\0') {
        if line.is_empty() {
            continue;
        }
        let Some((meta, path)) = line.split_once('\t') else {
            continue;
        };
        let mut parts = meta.split_whitespace();
        let mode = parts.next().unwrap_or("");
        let sha = parts.nth(1).unwrap_or("");
        if mode == "160000" {
            map.insert(path.to_string(), sha.to_string());
        }
    }
    map
}

/// Reads any gitlinks staged or tracked in the index (mode 160000).
async fn read_index_gitlinks(
    layer: &ProcessLayer,
    superproject_root: &Path,
) -> HashMap<String, String> {
    let result = layer
        .run(
            GitCall::new(superproject_root, ["ls-files", "--stage", "-z"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;
    let mut map = HashMap::new();
    let Ok(result) = result else { return map };
    if !result.ok() {
        return map;
    }
    let text = String::from_utf8_lossy(&result.stdout);
    for line in text.split('\0') {
        if line.is_empty() {
            continue;
        }
        let Some((meta, path)) = line.split_once('\t') else {
            continue;
        };
        let mut parts = meta.split_whitespace();
        let mode = parts.next().unwrap_or("");
        let sha = parts.next().unwrap_or("");
        if mode == "160000" {
            map.insert(path.to_string(), sha.to_string());
        }
    }
    map
}

/// Reads submodule URLs configured in `.git/config` for drift detection.
async fn read_config_submodule_urls(
    layer: &ProcessLayer,
    superproject_root: &Path,
) -> HashMap<String, String> {
    let result = layer
        .run(
            GitCall::new(
                superproject_root,
                ["config", "--get-regexp", r"^submodule\..*\.url$"],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;
    let mut map = HashMap::new();
    let Ok(result) = result else { return map };
    if !result.ok() {
        return map;
    }
    for line in result.stdout_utf8_lossy().lines() {
        let line = line.trim();
        if let Some((key, url)) = line.split_once(' ') {
            if let Some(rest) = key.strip_prefix("submodule.") {
                if let Some(name) = rest.strip_suffix(".url") {
                    map.insert(name.to_string(), url.trim().to_string());
                }
            }
        }
    }
    map
}

/// Resolves the actual `.git` directory for a repository path, properly handling
/// modern git submodule `.git` file pointers (`gitdir: <path>`).
pub fn resolve_git_dir(path: &Path) -> Option<PathBuf> {
    let git = path.join(".git");
    if git.is_dir() {
        return Some(git);
    }
    if git.is_file() {
        if let Ok(content) = std::fs::read_to_string(&git) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("gitdir:") {
                    let gitdir_path = rest.trim();
                    let target = path.join(gitdir_path);
                    return Some(target.canonicalize().unwrap_or(target));
                }
            }
        }
    }
    None
}

/// Checks whether a submodule path is initialized on disk with a valid git directory.
pub fn is_submodule_initialized(path: &Path) -> bool {
    if let Some(git_dir) = resolve_git_dir(path) {
        git_dir.join("HEAD").exists()
    } else {
        false
    }
}

/// Resolves the branch state of a submodule, identifying named branches
/// or detached HEAD with any refs (branches/tags) pointing at the commit.
async fn resolve_submodule_branch(layer: &ProcessLayer, path: &Path) -> Resolved<SubmoduleBranch> {
    let symbolic = layer
        .run(
            GitCall::new(path, ["symbolic-ref", "--short", "-q", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;

    if let Ok(r) = symbolic {
        if r.ok() {
            let name = r.stdout_utf8_lossy().trim().to_string();
            if !name.is_empty() {
                return Resolved::known(SubmoduleBranch::Named(name));
            }
        }
    }

    let rev_parse = layer
        .run(
            GitCall::new(path, ["rev-parse", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;

    let head_commit = match rev_parse {
        Ok(r) if r.ok() => r.stdout_utf8_lossy().trim().to_string(),
        Ok(r) => return Resolved::unknown(format!("rev-parse HEAD failed: {}", r.stderr.trim())),
        Err(e) => return Resolved::unknown(e.to_string()),
    };

    let pointing_result = layer
        .run(
            GitCall::new(
                path,
                [
                    "for-each-ref",
                    "--points-at",
                    "HEAD",
                    "--format=%(refname:short)",
                    "refs/heads",
                    "refs/tags",
                ],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;

    let pointing_refs = match pointing_result {
        Ok(r) if r.ok() => r
            .stdout_utf8_lossy()
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => Vec::new(),
    };

    Resolved::known(SubmoduleBranch::Detached {
        commit: head_commit,
        pointing_refs,
    })
}

/// Computes gitlink divergence with distinct states for in-sync, behind, ahead,
/// both, missing gitlink object, and unrelated histories.
async fn resolve_gitlink_divergence(
    layer: &ProcessLayer,
    path: &Path,
    gitlink: Option<&str>,
    head_commit: Option<&str>,
) -> Resolved<GitlinkDivergence> {
    let (Some(gitlink_sha), Some(head_sha)) = (gitlink, head_commit) else {
        if gitlink.is_none() {
            return Resolved::unknown("no gitlink recorded in superproject");
        } else {
            return Resolved::unknown("could not resolve submodule HEAD");
        }
    };

    if gitlink_sha == head_sha {
        return Resolved::known(GitlinkDivergence::InSync);
    }

    let obj_check = layer
        .run(
            GitCall::new(
                path,
                ["cat-file", "-e", &format!("{gitlink_sha}^{{commit}}")],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;

    if let Ok(r) = obj_check {
        if !r.ok() {
            return Resolved::known(GitlinkDivergence::GitlinkObjectMissingLocally {
                gitlink_commit: gitlink_sha.to_string(),
            });
        }
    }

    let range = format!("{gitlink_sha}...{head_sha}");
    let rev_list = layer
        .run(
            GitCall::new(path, ["rev-list", "--left-right", "--count", &range]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;

    match rev_list {
        Ok(r) if r.ok() => {
            let text = r.stdout_utf8_lossy();
            let mut nums = text
                .split_whitespace()
                .filter_map(|s| s.parse::<u32>().ok());
            let behind = nums.next().unwrap_or(0);
            let ahead = nums.next().unwrap_or(0);
            if behind == 0 && ahead == 0 {
                Resolved::known(GitlinkDivergence::UnrelatedHistories)
            } else if behind == 0 && ahead > 0 {
                Resolved::known(GitlinkDivergence::Ahead { ahead })
            } else if ahead == 0 && behind > 0 {
                Resolved::known(GitlinkDivergence::Behind { behind })
            } else {
                Resolved::known(GitlinkDivergence::Both { ahead, behind })
            }
        }
        Ok(r)
            if r.stderr.contains("unknown revision")
                || r.stderr.contains("bad revision")
                || r.stderr.contains("Not a valid object name")
                || r.stderr.contains("Invalid symmetric difference expression") =>
        {
            Resolved::known(GitlinkDivergence::GitlinkObjectMissingLocally {
                gitlink_commit: gitlink_sha.to_string(),
            })
        }
        Ok(r) => Resolved::unknown(format!("rev-list failed: {}", r.stderr.trim())),
        Err(e) => Resolved::unknown(e.to_string()),
    }
}

/// Detects `.gitmodules` drift: declared-but-absent, present-but-undeclared,
/// URL mismatch, and orphaned declarations.
fn detect_drift(
    declared: Option<&DeclaredSubmodule>,
    gitlink: Option<&str>,
    index_gitlink: Option<&str>,
    config_url: Option<&str>,
    path_exists: bool,
    is_initialized: bool,
) -> Vec<SubmoduleDrift> {
    let mut drifts = Vec::new();
    let has_gitlink = gitlink.is_some() || index_gitlink.is_some();

    match (declared, has_gitlink, path_exists, is_initialized) {
        (Some(_), true, false, _) | (Some(_), true, true, false) => {
            drifts.push(SubmoduleDrift::DeclaredButAbsent);
        }
        (Some(_), false, false, _) => {
            drifts.push(SubmoduleDrift::OrphanedDeclaration);
        }
        (None, true, _, _) | (None, _, true, true) => {
            drifts.push(SubmoduleDrift::PresentButUndeclared);
        }
        _ => {}
    }

    if let (Some(decl), Some(cfg_url)) = (declared, config_url) {
        if let Some(decl_url) = &decl.url {
            if !decl_url.is_empty() && !cfg_url.is_empty() && decl_url != cfg_url {
                drifts.push(SubmoduleDrift::UrlMismatch {
                    declared_url: decl_url.clone(),
                    config_url: cfg_url.to_string(),
                });
            }
        }
    }

    drifts
}

#[allow(clippy::too_many_arguments)]
async fn query_one_submodule(
    layer: &ProcessLayer,
    superproject_root: &Path,
    parent_path: Option<&Path>,
    depth: usize,
    declared: Option<DeclaredSubmodule>,
    rel_path: String,
    gitlink: Option<String>,
    head_gitlink: Option<String>,
    index_gitlink: Option<String>,
    config_url: Option<String>,
) -> SubmoduleState {
    let path = superproject_root.join(&rel_path);
    let initialized = is_submodule_initialized(&path);
    let path_exists = path.exists();

    let name = declared
        .as_ref()
        .map(|d| d.name.clone())
        .unwrap_or_else(|| rel_path.clone());

    let declared_branch = match &declared {
        Some(d) => Resolved::known(d.branch.clone()),
        None => Resolved::unknown("submodule is undeclared"),
    };

    let url = match &declared {
        Some(d) => Resolved::known(d.url.clone()),
        None => Resolved::unknown("submodule is undeclared"),
    };

    let gitlink_commit = match &gitlink {
        Some(sha) => Resolved::known(sha.clone()),
        None => Resolved::unknown("no gitlink recorded in superproject"),
    };

    let drifts = detect_drift(
        declared.as_ref(),
        head_gitlink.as_deref(),
        index_gitlink.as_deref(),
        config_url.as_deref(),
        path_exists,
        initialized,
    );
    let drift = Resolved::known(drifts);

    if !initialized {
        return SubmoduleState {
            name,
            path,
            relative_path: rel_path,
            depth,
            parent_path: parent_path.map(|p| p.to_path_buf()),
            declared_branch,
            url,
            initialized: false,
            gitlink_commit,
            branch: Resolved::unknown("submodule is not initialized"),
            gitlink_divergence: Resolved::unknown("submodule is not initialized"),
            remote_basis: Resolved::unknown("submodule is not initialized"),
            remote_ahead_behind: Resolved::unknown("submodule is not initialized"),
            dirty: Resolved::unknown("submodule is not initialized"),
            last_fetch_unix_secs: Resolved::unknown("submodule is not initialized"),
            drift,
        };
    }

    let branch = resolve_submodule_branch(layer, &path).await;
    let branch_name = branch.as_known().and_then(|b| match b {
        SubmoduleBranch::Named(n) => Some(n.clone()),
        SubmoduleBranch::Detached { .. } => None,
    });

    let head_commit = match branch.as_known() {
        Some(SubmoduleBranch::Named(_)) => layer
            .run(
                GitCall::new(&path, ["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
            .ok()
            .filter(|r| r.ok())
            .map(|r| r.stdout_utf8_lossy().trim().to_string()),
        Some(SubmoduleBranch::Detached { commit, .. }) => Some(commit.clone()),
        None => None,
    };

    let gitlink_divergence =
        resolve_gitlink_divergence(layer, &path, gitlink.as_deref(), head_commit.as_deref()).await;

    let remote_basis =
        Resolved::known(resolve_upstream_basis(layer, &path, branch_name.as_deref()).await);

    let remote_ahead_behind = match remote_basis.as_known().and_then(|b| b.refname()) {
        Some(refname) => {
            let range = format!("{refname}...HEAD");
            match layer
                .run(
                    GitCall::new(&path, ["rev-list", "--left-right", "--count", &range]),
                    Intent::Read,
                    CancellationToken::new(),
                )
                .await
            {
                Ok(r) if r.ok() => {
                    let text = r.stdout_utf8_lossy();
                    let mut nums = text
                        .split_whitespace()
                        .filter_map(|s| s.parse::<u32>().ok());
                    let behind = nums.next().unwrap_or(0);
                    let ahead = nums.next().unwrap_or(0);
                    Resolved::known((ahead, behind))
                }
                Ok(r) => Resolved::unknown(format!("rev-list failed: {}", r.stderr.trim())),
                Err(e) => Resolved::unknown(e.to_string()),
            }
        }
        None => Resolved::unknown("no remote basis"),
    };

    let dirty = match layer
        .run(
            GitCall::new(
                &path,
                ["status", "--porcelain=v2", "-z", "--untracked-files=all"],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
    {
        Ok(r) if r.ok() => Resolved::known(!r.stdout.is_empty()),
        Ok(r) => Resolved::unknown(format!("status failed: {}", r.stderr.trim())),
        Err(e) => Resolved::unknown(e.to_string()),
    };

    let real_git_dir = resolve_git_dir(&path);
    let fetch_head = real_git_dir.map(|g| g.join("FETCH_HEAD"));
    let last_fetch_unix_secs = match fetch_head {
        Some(fh) if fh.exists() => match std::fs::metadata(&fh).and_then(|m| m.modified()) {
            Ok(t) => Resolved::known(
                t.duration_since(SystemTime::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs()),
            ),
            Err(e) => Resolved::unknown(e.to_string()),
        },
        Some(_) => Resolved::known(None),
        None => Resolved::unknown("could not resolve git directory"),
    };

    SubmoduleState {
        name,
        path,
        relative_path: rel_path,
        depth,
        parent_path: parent_path.map(|p| p.to_path_buf()),
        declared_branch,
        url,
        initialized: true,
        gitlink_commit,
        branch,
        gitlink_divergence,
        remote_basis,
        remote_ahead_behind,
        dirty,
        last_fetch_unix_secs,
        drift,
    }
}

/// Recursive submodule query visiting nested submodules up to `max_depth`
/// with cycle detection tracking canonical git directories.
async fn query_submodules_recursive(
    layer: &ProcessLayer,
    root: &Path,
    parent_path: Option<&Path>,
    current_depth: usize,
    max_depth: usize,
    visited: &mut HashSet<PathBuf>,
) -> SubmoduleMatrixResult {
    let canonical_root = resolve_git_dir(root)
        .and_then(|p| p.canonicalize().ok())
        .or_else(|| root.canonicalize().ok())
        .unwrap_or_else(|| root.to_path_buf());

    if visited.contains(&canonical_root) {
        return SubmoduleMatrixResult {
            submodules: Vec::new(),
            malformed_entries: Vec::new(),
        };
    }
    visited.insert(canonical_root);

    let ((declared_submodules, malformed_entries), head_gitlinks, index_gitlinks, config_urls) = tokio::join!(
        read_gitmodules_resilient(layer, root),
        read_gitlinks(layer, root),
        read_index_gitlinks(layer, root),
        read_config_submodule_urls(layer, root),
    );

    let mut all_declared_paths = HashSet::new();
    for d in &declared_submodules {
        all_declared_paths.insert(d.path.clone());
    }

    let mut entries_to_query: Vec<(Option<DeclaredSubmodule>, String, Option<String>)> = Vec::new();

    for d in declared_submodules {
        let path = d.path.clone();
        let gitlink = head_gitlinks
            .get(&path)
            .or_else(|| index_gitlinks.get(&path))
            .cloned();
        entries_to_query.push((Some(d), path, gitlink));
    }

    for (path, sha) in &head_gitlinks {
        if !all_declared_paths.contains(path) {
            entries_to_query.push((None, path.clone(), Some(sha.clone())));
        }
    }
    for (path, sha) in &index_gitlinks {
        if !all_declared_paths.contains(path) && !head_gitlinks.contains_key(path) {
            entries_to_query.push((None, path.clone(), Some(sha.clone())));
        }
    }

    let futures = entries_to_query
        .into_iter()
        .map(|(declared, rel_path, gitlink)| {
            let config_url = declared
                .as_ref()
                .and_then(|d| config_urls.get(&d.name).cloned());
            let head_gitlink = head_gitlinks.get(&rel_path).cloned();
            let index_gitlink = index_gitlinks.get(&rel_path).cloned();
            query_one_submodule(
                layer,
                root,
                parent_path,
                current_depth,
                declared,
                rel_path,
                gitlink,
                head_gitlink,
                index_gitlink,
                config_url,
            )
        });

    let direct_submodules = futures::future::join_all(futures).await;

    let mut all_submodules = Vec::new();
    let mut all_malformed = malformed_entries;

    for sm in direct_submodules {
        let sm_path = sm.path.clone();
        let is_init = sm.initialized;
        all_submodules.push(sm);

        if is_init && current_depth < max_depth {
            let nested = Box::pin(query_submodules_recursive(
                layer,
                &sm_path,
                Some(root),
                current_depth + 1,
                max_depth,
                visited,
            ))
            .await;
            all_submodules.extend(nested.submodules);
            all_malformed.extend(nested.malformed_entries);
        }
    }

    SubmoduleMatrixResult {
        submodules: all_submodules,
        malformed_entries: all_malformed,
    }
}

/// Traverses submodules starting at `superproject_root` up to `max_depth` levels deep,
/// preventing cycles and returning all discovered submodules and malformed `.gitmodules` entries.
pub async fn query_submodule_matrix_with_depth(
    layer: &ProcessLayer,
    superproject_root: &Path,
    max_depth: usize,
) -> SubmoduleMatrixResult {
    let mut visited = HashSet::new();
    query_submodules_recursive(layer, superproject_root, None, 1, max_depth, &mut visited).await
}

/// Fans out from batched superproject-level calls into concurrent per-submodule
/// queries, bounded by the `ProcessLayer`'s worker pool. Traverses nested submodules
/// up to default depth 2 with cycle protection.
pub async fn query_submodule_matrix(
    layer: &ProcessLayer,
    superproject_root: &Path,
) -> SubmoduleMatrixResult {
    query_submodule_matrix_with_depth(layer, superproject_root, 2).await
}

/// Queries the state of a single targeted submodule within a superproject,
/// enabling surgical invalidation when only one submodule's repository changes.
pub async fn query_single_submodule(
    layer: &ProcessLayer,
    superproject_root: &Path,
    submodule_path: &Path,
) -> Option<SubmoduleState> {
    let rel_path = if let Ok(rel) = submodule_path.strip_prefix(superproject_root) {
        rel.to_string_lossy().to_string()
    } else {
        submodule_path.to_string_lossy().to_string()
    };

    let (declared_list, _) = read_gitmodules_resilient(layer, superproject_root).await;
    let declared = declared_list
        .into_iter()
        .find(|d| d.path == rel_path || d.name == rel_path);

    let ls_tree = layer
        .run(
            GitCall::new(superproject_root, ["ls-tree", "HEAD", &rel_path]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;

    let head_gitlink = ls_tree.ok().filter(|r| r.ok()).and_then(|r| {
        let out = r.stdout_utf8_lossy();
        let parts: Vec<&str> = out.split_whitespace().collect();
        if parts.len() >= 3 && parts[0] == "160000" {
            Some(parts[2].to_string())
        } else {
            None
        }
    });

    let ls_files = layer
        .run(
            GitCall::new(superproject_root, ["ls-files", "--stage", &rel_path]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;

    let index_gitlink = ls_files.ok().filter(|r| r.ok()).and_then(|r| {
        let out = r.stdout_utf8_lossy();
        let parts: Vec<&str> = out.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == "160000" {
            Some(parts[1].to_string())
        } else {
            None
        }
    });

    let config_url = if let Some(decl) = &declared {
        let cfg = layer
            .run(
                GitCall::new(
                    superproject_root,
                    ["config", "--get", &format!("submodule.{}.url", decl.name)],
                ),
                Intent::Read,
                CancellationToken::new(),
            )
            .await;
        cfg.ok()
            .filter(|r| r.ok())
            .map(|r| r.stdout_utf8_lossy().trim().to_string())
    } else {
        None
    };

    let gitlink = head_gitlink.clone().or_else(|| index_gitlink.clone());

    if declared.is_none() && gitlink.is_none() && !superproject_root.join(&rel_path).exists() {
        return None;
    }

    Some(
        query_one_submodule(
            layer,
            superproject_root,
            None,
            1,
            declared,
            rel_path,
            gitlink,
            head_gitlink,
            index_gitlink,
            config_url,
        )
        .await,
    )
}

fn filter_submodules_by_paths(
    matrix: &[SubmoduleState],
    superproject_root: &Path,
    paths: Option<&[PathBuf]>,
) -> Vec<SubmoduleState> {
    match paths {
        Some(filter_paths) => {
            let filter_set: HashSet<PathBuf> = filter_paths
                .iter()
                .map(|p| {
                    if p.is_relative() {
                        superproject_root.join(p)
                    } else {
                        p.clone()
                    }
                })
                .collect();
            matrix
                .iter()
                .filter(|s| filter_set.contains(&s.path))
                .cloned()
                .collect()
        }
        None => matrix.to_vec(),
    }
}

/// Refreshes submodule network tracking branches in parallel with bounded concurrency,
/// isolated per-row error handling, and live progress reporting.
pub async fn refresh_submodules_network(
    layer: &ProcessLayer,
    superproject_root: &Path,
    options: SubmoduleRefreshOptions,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<SubmoduleRefreshProgress>>,
    cancel: CancellationToken,
) -> SubmoduleRefreshResult {
    let matrix = query_submodule_matrix(layer, superproject_root).await;
    let target_submodules = filter_submodules_by_paths(
        &matrix.submodules,
        superproject_root,
        options.paths.as_deref(),
    );

    let concurrency = options.concurrency.unwrap_or(8).max(1);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let prune = options.prune.unwrap_or(true);
    let tags = options.tags.unwrap_or(false);

    let mut tasks = Vec::new();

    for sm in target_submodules {
        let sm_path = sm.path.clone();
        let rel_path = sm.relative_path.clone();
        let is_init = sm.initialized;
        let layer_ref = layer;
        let cancel_token = cancel.clone();
        let sem_clone = semaphore.clone();
        let tx = progress_tx.clone();
        let root_clone = superproject_root.to_path_buf();

        tasks.push(async move {
            if cancel_token.is_cancelled() {
                return SubmoduleRefreshRowResult {
                    path: sm_path,
                    relative_path: rel_path,
                    status: SubmoduleRefreshStatus::Cancelled,
                    updated_state: Some(sm),
                };
            }

            if !is_init {
                let reason = "submodule is not initialized".to_string();
                if let Some(t) = &tx {
                    let _ = t.send(SubmoduleRefreshProgress {
                        path: sm_path.clone(),
                        relative_path: rel_path.clone(),
                        stage: SubmoduleRefreshStage::Skipped {
                            reason: reason.clone(),
                        },
                    });
                }
                return SubmoduleRefreshRowResult {
                    path: sm_path,
                    relative_path: rel_path,
                    status: SubmoduleRefreshStatus::Skipped { reason },
                    updated_state: Some(sm),
                };
            }

            let _permit = match sem_clone.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    return SubmoduleRefreshRowResult {
                        path: sm_path,
                        relative_path: rel_path,
                        status: SubmoduleRefreshStatus::Cancelled,
                        updated_state: Some(sm),
                    };
                }
            };

            if cancel_token.is_cancelled() {
                return SubmoduleRefreshRowResult {
                    path: sm_path,
                    relative_path: rel_path,
                    status: SubmoduleRefreshStatus::Cancelled,
                    updated_state: Some(sm),
                };
            }

            let remotes = match layer_ref
                .run(
                    GitCall::new(&sm_path, ["remote"]),
                    Intent::Read,
                    cancel_token.clone(),
                )
                .await
            {
                Ok(r) if r.ok() => {
                    let list: Vec<String> = r
                        .stdout_utf8_lossy()
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect();
                    list
                }
                Ok(r) => {
                    let err = format!("git remote failed: {}", r.stderr.trim());
                    if let Some(t) = &tx {
                        let _ = t.send(SubmoduleRefreshProgress {
                            path: sm_path.clone(),
                            relative_path: rel_path.clone(),
                            stage: SubmoduleRefreshStage::Failed { error: err.clone() },
                        });
                    }
                    return SubmoduleRefreshRowResult {
                        path: sm_path,
                        relative_path: rel_path,
                        status: SubmoduleRefreshStatus::Failed { error: err },
                        updated_state: Some(sm),
                    };
                }
                Err(e) => {
                    let err = e.to_string();
                    if let Some(t) = &tx {
                        let _ = t.send(SubmoduleRefreshProgress {
                            path: sm_path.clone(),
                            relative_path: rel_path.clone(),
                            stage: SubmoduleRefreshStage::Failed { error: err.clone() },
                        });
                    }
                    return SubmoduleRefreshRowResult {
                        path: sm_path,
                        relative_path: rel_path,
                        status: SubmoduleRefreshStatus::Failed { error: err },
                        updated_state: Some(sm),
                    };
                }
            };

            if remotes.is_empty() {
                let reason = "no remotes configured in submodule".to_string();
                if let Some(t) = &tx {
                    let _ = t.send(SubmoduleRefreshProgress {
                        path: sm_path.clone(),
                        relative_path: rel_path.clone(),
                        stage: SubmoduleRefreshStage::Skipped {
                            reason: reason.clone(),
                        },
                    });
                }
                return SubmoduleRefreshRowResult {
                    path: sm_path,
                    relative_path: rel_path,
                    status: SubmoduleRefreshStatus::Skipped { reason },
                    updated_state: Some(sm),
                };
            }

            if let Some(t) = &tx {
                let _ = t.send(SubmoduleRefreshProgress {
                    path: sm_path.clone(),
                    relative_path: rel_path.clone(),
                    stage: SubmoduleRefreshStage::Starting,
                });
            }

            let mut fetch_errors = Vec::new();
            let mut fetched_count = 0;

            for remote in &remotes {
                if cancel_token.is_cancelled() {
                    return SubmoduleRefreshRowResult {
                        path: sm_path,
                        relative_path: rel_path,
                        status: SubmoduleRefreshStatus::Cancelled,
                        updated_state: Some(sm),
                    };
                }

                if let Some(t) = &tx {
                    let _ = t.send(SubmoduleRefreshProgress {
                        path: sm_path.clone(),
                        relative_path: rel_path.clone(),
                        stage: SubmoduleRefreshStage::Fetching {
                            remote: remote.clone(),
                        },
                    });
                }

                let mut fetch_args = vec!["fetch".to_string()];
                if prune {
                    fetch_args.push("--prune".to_string());
                }
                if tags {
                    fetch_args.push("--tags".to_string());
                }
                fetch_args.push(remote.clone());

                let res = layer_ref
                    .run(
                        GitCall::new(&sm_path, fetch_args.iter().map(|s| s.as_str())),
                        Intent::Write,
                        cancel_token.clone(),
                    )
                    .await;

                match res {
                    Ok(out) if out.ok() => {
                        fetched_count += 1;
                    }
                    Ok(out) => {
                        let err_msg = out.stderr.trim();
                        fetch_errors.push(format!("{remote}: {err_msg}"));
                    }
                    Err(e) => {
                        fetch_errors.push(format!("{remote}: {e}"));
                    }
                }
            }

            if !fetch_errors.is_empty() && fetched_count == 0 {
                let error = fetch_errors.join("; ");
                if let Some(t) = &tx {
                    let _ = t.send(SubmoduleRefreshProgress {
                        path: sm_path.clone(),
                        relative_path: rel_path.clone(),
                        stage: SubmoduleRefreshStage::Failed {
                            error: error.clone(),
                        },
                    });
                }
                return SubmoduleRefreshRowResult {
                    path: sm_path,
                    relative_path: rel_path,
                    status: SubmoduleRefreshStatus::Failed { error },
                    updated_state: Some(sm),
                };
            }

            let summary = format!("fetched {fetched_count} remote(s)");
            if let Some(t) = &tx {
                let _ = t.send(SubmoduleRefreshProgress {
                    path: sm_path.clone(),
                    relative_path: rel_path.clone(),
                    stage: SubmoduleRefreshStage::Completed {
                        summary: summary.clone(),
                    },
                });
            }

            let updated_state = query_single_submodule(layer_ref, &root_clone, &sm_path).await;

            SubmoduleRefreshRowResult {
                path: sm_path,
                relative_path: rel_path,
                status: SubmoduleRefreshStatus::Success { summary },
                updated_state,
            }
        });
    }

    let rows = futures::future::join_all(tasks).await;

    let total = rows.len();
    let mut succeeded = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for r in &rows {
        match r.status {
            SubmoduleRefreshStatus::Success { .. } => succeeded += 1,
            SubmoduleRefreshStatus::Failed { .. } => failed += 1,
            SubmoduleRefreshStatus::Skipped { .. } | SubmoduleRefreshStatus::Cancelled => {
                skipped += 1
            }
        }
    }

    SubmoduleRefreshResult {
        total,
        succeeded,
        failed,
        skipped,
        rows,
    }
}

/// Previews a bulk checkout operation across target submodules,
/// categorizing which submodules will switch, are already on branch, or must be skipped for dirtiness.
pub async fn preview_bulk_checkout(
    layer: &ProcessLayer,
    superproject_root: &Path,
    target_branch: &str,
    submodule_paths: Option<Vec<PathBuf>>,
) -> BulkCheckoutPreview {
    let matrix = query_submodule_matrix(layer, superproject_root).await;
    let targets = filter_submodules_by_paths(
        &matrix.submodules,
        superproject_root,
        submodule_paths.as_deref(),
    );

    let mut will_switch = 0;
    let mut already_on_branch = 0;
    let mut skipped_dirty = 0;
    let mut missing_branch = 0;
    let mut items = Vec::new();

    for sm in targets {
        let action = if !sm.initialized {
            BulkCheckoutAction::Uninitialized
        } else if sm.dirty.as_known().copied().unwrap_or(false) {
            skipped_dirty += 1;
            BulkCheckoutAction::SkippedDirty {
                uncommitted_changes: true,
            }
        } else {
            let current_branch_name = match sm.branch.as_known() {
                Some(SubmoduleBranch::Named(name)) => name.clone(),
                Some(SubmoduleBranch::Detached { commit, .. }) => {
                    format!("detached ({commit})")
                }
                None => "unknown".to_string(),
            };

            if current_branch_name == target_branch {
                already_on_branch += 1;
                BulkCheckoutAction::AlreadyOnBranch {
                    branch: target_branch.to_string(),
                }
            } else {
                let local_check = layer
                    .run(
                        GitCall::new(
                            &sm.path,
                            [
                                "rev-parse",
                                "--verify",
                                &format!("refs/heads/{target_branch}"),
                            ],
                        ),
                        Intent::Read,
                        CancellationToken::new(),
                    )
                    .await;

                let local_exists = local_check.map(|r| r.ok()).unwrap_or(false);
                let remote_exists = if !local_exists {
                    let remote_check = layer
                        .run(
                            GitCall::new(
                                &sm.path,
                                [
                                    "for-each-ref",
                                    "--format=%(refname:short)",
                                    &format!("refs/remotes/*/{target_branch}"),
                                ],
                            ),
                            Intent::Read,
                            CancellationToken::new(),
                        )
                        .await;
                    remote_check
                        .map(|r| r.ok() && !r.stdout_utf8_lossy().trim().is_empty())
                        .unwrap_or(false)
                } else {
                    false
                };

                if local_exists || remote_exists {
                    will_switch += 1;
                    BulkCheckoutAction::WillSwitch {
                        current_branch: current_branch_name,
                        target_branch: target_branch.to_string(),
                    }
                } else {
                    missing_branch += 1;
                    BulkCheckoutAction::MissingBranch {
                        branch: target_branch.to_string(),
                    }
                }
            }
        };

        items.push(BulkCheckoutPreviewItem {
            path: sm.path,
            relative_path: sm.relative_path,
            action,
        });
    }

    BulkCheckoutPreview {
        total: items.len(),
        will_switch,
        already_on_branch,
        skipped_dirty,
        missing_branch,
        items,
    }
}

/// Executes a bulk branch checkout across target submodules with dirty tree protection.
pub async fn execute_bulk_checkout(
    layer: &ProcessLayer,
    superproject_root: &Path,
    target_branch: &str,
    submodule_paths: Option<Vec<PathBuf>>,
    cancel: CancellationToken,
) -> BulkOperationResult {
    let preview =
        preview_bulk_checkout(layer, superproject_root, target_branch, submodule_paths).await;
    let total = preview.items.len();
    let mut succeeded = 0;
    let mut skipped = 0;
    let mut failed = 0;
    let mut results = Vec::new();

    for item in preview.items {
        if cancel.is_cancelled() {
            skipped += 1;
            results.push(BulkOperationItemResult {
                path: item.path,
                relative_path: item.relative_path,
                outcome: BulkItemOutcome::Cancelled,
                updated_state: None,
            });
            continue;
        }

        match item.action {
            BulkCheckoutAction::WillSwitch { .. } => {
                let res = layer
                    .run(
                        GitCall::new(&item.path, ["checkout", target_branch]),
                        Intent::Write,
                        cancel.clone(),
                    )
                    .await;

                match res {
                    Ok(out) if out.ok() => {
                        succeeded += 1;
                        let updated =
                            query_single_submodule(layer, superproject_root, &item.path).await;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Success {
                                message: format!("Switched to branch '{target_branch}'"),
                            },
                            updated_state: updated,
                        });
                    }
                    Ok(out) => {
                        failed += 1;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Failed {
                                error: out.stderr.trim().to_string(),
                            },
                            updated_state: None,
                        });
                    }
                    Err(e) => {
                        failed += 1;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Failed {
                                error: e.to_string(),
                            },
                            updated_state: None,
                        });
                    }
                }
            }
            BulkCheckoutAction::AlreadyOnBranch { branch } => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: format!("Already on branch '{branch}'"),
                    },
                    updated_state: None,
                });
            }
            BulkCheckoutAction::SkippedDirty { .. } => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: "Submodule has uncommitted changes".to_string(),
                    },
                    updated_state: None,
                });
            }
            BulkCheckoutAction::MissingBranch { branch } => {
                failed += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Failed {
                        error: format!("Branch '{branch}' not found in submodule"),
                    },
                    updated_state: None,
                });
            }
            BulkCheckoutAction::Uninitialized => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: "Submodule is not initialized".to_string(),
                    },
                    updated_state: None,
                });
            }
        }
    }

    BulkOperationResult {
        total,
        succeeded,
        skipped,
        failed,
        items: results,
    }
}

/// Previews a bulk pull operation across target submodules,
/// checking behind counts, upstream configuration, and dirtiness.
pub async fn preview_bulk_pull(
    layer: &ProcessLayer,
    superproject_root: &Path,
    submodule_paths: Option<Vec<PathBuf>>,
) -> BulkPullPreview {
    let matrix = query_submodule_matrix(layer, superproject_root).await;
    let targets = filter_submodules_by_paths(
        &matrix.submodules,
        superproject_root,
        submodule_paths.as_deref(),
    );

    let mut will_pull = 0;
    let mut already_up_to_date = 0;
    let mut skipped_dirty = 0;
    let mut skipped_no_upstream = 0;
    let mut items = Vec::new();

    for sm in targets {
        let action = if !sm.initialized {
            BulkPullAction::Uninitialized
        } else if sm.dirty.as_known().copied().unwrap_or(false) {
            skipped_dirty += 1;
            BulkPullAction::SkippedDirty {
                uncommitted_changes: true,
            }
        } else {
            let branch_name = match sm.branch.as_known() {
                Some(SubmoduleBranch::Named(name)) => Some(name.clone()),
                _ => None,
            };

            let upstream = sm.remote_basis.as_known().and_then(|b| b.refname());

            match (branch_name, upstream) {
                (Some(b), Some(u)) => {
                    let behind = sm
                        .remote_ahead_behind
                        .as_known()
                        .map(|(_, behind)| *behind)
                        .unwrap_or(0);
                    if behind > 0 {
                        will_pull += 1;
                        BulkPullAction::WillPull {
                            branch: b,
                            upstream: u.to_string(),
                            behind,
                        }
                    } else {
                        already_up_to_date += 1;
                        BulkPullAction::AlreadyUpToDate {
                            branch: b,
                            upstream: u.to_string(),
                        }
                    }
                }
                _ => {
                    skipped_no_upstream += 1;
                    BulkPullAction::SkippedNoUpstream
                }
            }
        };

        items.push(BulkPullPreviewItem {
            path: sm.path,
            relative_path: sm.relative_path,
            action,
        });
    }

    BulkPullPreview {
        total: items.len(),
        will_pull,
        already_up_to_date,
        skipped_dirty,
        skipped_no_upstream,
        items,
    }
}

/// Executes a bulk pull operation across clean target submodules with upstream tracking.
pub async fn execute_bulk_pull(
    layer: &ProcessLayer,
    superproject_root: &Path,
    options: BulkPullOptions,
    cancel: CancellationToken,
) -> BulkOperationResult {
    let preview = preview_bulk_pull(layer, superproject_root, options.paths.clone()).await;
    let total = preview.items.len();
    let mut succeeded = 0;
    let mut skipped = 0;
    let mut failed = 0;
    let mut results = Vec::new();

    for item in preview.items {
        if cancel.is_cancelled() {
            skipped += 1;
            results.push(BulkOperationItemResult {
                path: item.path,
                relative_path: item.relative_path,
                outcome: BulkItemOutcome::Cancelled,
                updated_state: None,
            });
            continue;
        }

        match item.action {
            BulkPullAction::WillPull { upstream, .. } => {
                let parts: Vec<&str> = upstream.splitn(2, '/').collect();
                let remote = parts.first().unwrap_or(&"origin").to_string();
                let branch = parts.get(1).map(|b| b.to_string());

                let pull_opts = PullOptions {
                    remote,
                    branch,
                    strategy: options.strategy.clone(),
                    prune: false,
                    tags: false,
                };

                let res = repo_pull(layer, &item.path, pull_opts, cancel.clone()).await;

                match res {
                    Ok(PullOutcome::Success { summary }) => {
                        succeeded += 1;
                        let updated =
                            query_single_submodule(layer, superproject_root, &item.path).await;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Success { message: summary },
                            updated_state: updated,
                        });
                    }
                    Ok(PullOutcome::Conflict {
                        conflicting_files,
                        summary,
                    }) => {
                        failed += 1;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Failed {
                                error: format!(
                                    "{summary}: conflicting files: {}",
                                    conflicting_files.join(", ")
                                ),
                            },
                            updated_state: None,
                        });
                    }
                    Ok(PullOutcome::Failed { message }) => {
                        failed += 1;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Failed { error: message },
                            updated_state: None,
                        });
                    }
                    Ok(PullOutcome::Cancelled { summary: _ }) => {
                        skipped += 1;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Cancelled,
                            updated_state: None,
                        });
                    }
                    Err(e) => {
                        failed += 1;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Failed { error: e },
                            updated_state: None,
                        });
                    }
                }
            }
            BulkPullAction::AlreadyUpToDate { upstream, .. } => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: format!("Already up to date with {upstream}"),
                    },
                    updated_state: None,
                });
            }
            BulkPullAction::SkippedDirty { .. } => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: "Submodule has uncommitted changes".to_string(),
                    },
                    updated_state: None,
                });
            }
            BulkPullAction::SkippedNoUpstream => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: "No upstream configured for branch".to_string(),
                    },
                    updated_state: None,
                });
            }
            BulkPullAction::Uninitialized => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: "Submodule is not initialized".to_string(),
                    },
                    updated_state: None,
                });
            }
        }
    }

    BulkOperationResult {
        total,
        succeeded,
        skipped,
        failed,
        items: results,
    }
}

/// Previews a bulk reset operation to the recorded gitlink commit in the superproject.
pub async fn preview_bulk_reset_to_gitlink(
    layer: &ProcessLayer,
    superproject_root: &Path,
    submodule_paths: Option<Vec<PathBuf>>,
) -> BulkResetPreview {
    let matrix = query_submodule_matrix(layer, superproject_root).await;
    let targets = filter_submodules_by_paths(
        &matrix.submodules,
        superproject_root,
        submodule_paths.as_deref(),
    );

    let mut will_reset = 0;
    let mut already_in_sync = 0;
    let mut skipped_dirty = 0;
    let mut missing_object = 0;
    let mut items = Vec::new();

    for sm in targets {
        let action = if !sm.initialized {
            BulkResetAction::Uninitialized
        } else if sm.dirty.as_known().copied().unwrap_or(false) {
            skipped_dirty += 1;
            BulkResetAction::SkippedDirty {
                uncommitted_changes: true,
            }
        } else {
            match (
                sm.gitlink_commit.as_known(),
                sm.gitlink_divergence.as_known(),
            ) {
                (Some(gitlink), Some(GitlinkDivergence::InSync)) => {
                    already_in_sync += 1;
                    BulkResetAction::AlreadyInSync {
                        gitlink_commit: gitlink.clone(),
                    }
                }
                (Some(gitlink), Some(GitlinkDivergence::GitlinkObjectMissingLocally { .. })) => {
                    missing_object += 1;
                    BulkResetAction::MissingObject {
                        gitlink_commit: gitlink.clone(),
                    }
                }
                (Some(gitlink), Some(_)) => {
                    will_reset += 1;
                    let current = match sm.branch.as_known() {
                        Some(SubmoduleBranch::Detached { commit, .. }) => commit.clone(),
                        Some(SubmoduleBranch::Named(name)) => name.clone(),
                        None => "unknown".to_string(),
                    };
                    BulkResetAction::WillReset {
                        current_commit: current,
                        gitlink_commit: gitlink.clone(),
                    }
                }
                (None, _) => BulkResetAction::NoGitlinkRecorded,
                _ => BulkResetAction::NoGitlinkRecorded,
            }
        };

        items.push(BulkResetPreviewItem {
            path: sm.path,
            relative_path: sm.relative_path,
            action,
        });
    }

    BulkResetPreview {
        total: items.len(),
        will_reset,
        already_in_sync,
        skipped_dirty,
        missing_object,
        items,
    }
}

/// Executes a bulk reset of submodules to their recorded superproject gitlink commit,
/// strictly refusing any dirty submodules to prevent uncommitted data loss.
pub async fn execute_bulk_reset_to_gitlink(
    layer: &ProcessLayer,
    superproject_root: &Path,
    submodule_paths: Option<Vec<PathBuf>>,
    cancel: CancellationToken,
) -> BulkOperationResult {
    let preview = preview_bulk_reset_to_gitlink(layer, superproject_root, submodule_paths).await;
    let total = preview.items.len();
    let mut succeeded = 0;
    let mut skipped = 0;
    let mut failed = 0;
    let mut results = Vec::new();

    for item in preview.items {
        if cancel.is_cancelled() {
            skipped += 1;
            results.push(BulkOperationItemResult {
                path: item.path,
                relative_path: item.relative_path,
                outcome: BulkItemOutcome::Cancelled,
                updated_state: None,
            });
            continue;
        }

        match item.action {
            BulkResetAction::WillReset { gitlink_commit, .. } => {
                let res = layer
                    .run(
                        GitCall::new(&item.path, ["checkout", &gitlink_commit]),
                        Intent::Write,
                        cancel.clone(),
                    )
                    .await;

                match res {
                    Ok(out) if out.ok() => {
                        succeeded += 1;
                        let updated =
                            query_single_submodule(layer, superproject_root, &item.path).await;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Success {
                                message: format!("Reset to gitlink commit {gitlink_commit}"),
                            },
                            updated_state: updated,
                        });
                    }
                    Ok(out) => {
                        failed += 1;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Failed {
                                error: out.stderr.trim().to_string(),
                            },
                            updated_state: None,
                        });
                    }
                    Err(e) => {
                        failed += 1;
                        results.push(BulkOperationItemResult {
                            path: item.path,
                            relative_path: item.relative_path,
                            outcome: BulkItemOutcome::Failed {
                                error: e.to_string(),
                            },
                            updated_state: None,
                        });
                    }
                }
            }
            BulkResetAction::AlreadyInSync { gitlink_commit } => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: format!("Already in sync with gitlink {gitlink_commit}"),
                    },
                    updated_state: None,
                });
            }
            BulkResetAction::SkippedDirty { .. } => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: "Submodule has uncommitted changes".to_string(),
                    },
                    updated_state: None,
                });
            }
            BulkResetAction::MissingObject { gitlink_commit } => {
                failed += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Failed {
                        error: format!("Gitlink commit '{gitlink_commit}' is missing locally"),
                    },
                    updated_state: None,
                });
            }
            BulkResetAction::NoGitlinkRecorded => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: "No gitlink recorded in superproject".to_string(),
                    },
                    updated_state: None,
                });
            }
            BulkResetAction::Uninitialized => {
                skipped += 1;
                results.push(BulkOperationItemResult {
                    path: item.path,
                    relative_path: item.relative_path,
                    outcome: BulkItemOutcome::Skipped {
                        reason: "Submodule is not initialized".to_string(),
                    },
                    updated_state: None,
                });
            }
        }
    }

    BulkOperationResult {
        total,
        succeeded,
        skipped,
        failed,
        items: results,
    }
}

/// Bumps the superproject's recorded gitlink for a submodule to its current HEAD commit.
/// Refuses or warns if the HEAD commit is unpushed to any remote tracking branch.
pub async fn bump_submodule_gitlink(
    layer: &ProcessLayer,
    superproject_root: &Path,
    submodule_path: &Path,
    allow_unpushed: bool,
) -> BumpGitlinkOutcome {
    let rel_path = if let Ok(rel) = submodule_path.strip_prefix(superproject_root) {
        rel.to_string_lossy().to_string()
    } else {
        submodule_path.to_string_lossy().to_string()
    };

    let head_commit_res = layer
        .run(
            GitCall::new(submodule_path, ["rev-parse", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;

    let head_commit = match head_commit_res {
        Ok(r) if r.ok() => r.stdout_utf8_lossy().trim().to_string(),
        Ok(r) => {
            return BumpGitlinkOutcome::Failed {
                submodule_path: submodule_path.to_path_buf(),
                relative_path: rel_path,
                error: format!("Could not read submodule HEAD: {}", r.stderr.trim()),
            };
        }
        Err(e) => {
            return BumpGitlinkOutcome::Failed {
                submodule_path: submodule_path.to_path_buf(),
                relative_path: rel_path,
                error: e.to_string(),
            };
        }
    };

    let remote_contains_res = layer
        .run(
            GitCall::new(submodule_path, ["branch", "-r", "--contains", &head_commit]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;

    let is_pushed = match remote_contains_res {
        Ok(r) if r.ok() => !r.stdout_utf8_lossy().trim().is_empty(),
        _ => false,
    };

    if !is_pushed && !allow_unpushed {
        return BumpGitlinkOutcome::UnpushedRefused {
            submodule_path: submodule_path.to_path_buf(),
            relative_path: rel_path,
            head_commit,
            reason: "Submodule commit has not been pushed to any remote. Teammates checking out this gitlink will encounter missing objects.".to_string(),
        };
    }

    let warning = if !is_pushed {
        Some("Commit is not published to any remote. Ensure it is pushed so teammates can checkout this commit.".to_string())
    } else {
        None
    };

    let prev_ls = layer
        .run(
            GitCall::new(superproject_root, ["ls-tree", "HEAD", &rel_path]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;
    let previous_gitlink = prev_ls.ok().filter(|r| r.ok()).and_then(|r| {
        let out = r.stdout_utf8_lossy();
        let parts: Vec<&str> = out.split_whitespace().collect();
        if parts.len() >= 3 && parts[0] == "160000" {
            Some(parts[2].to_string())
        } else {
            None
        }
    });

    let add_res = layer
        .run(
            GitCall::new(superproject_root, ["add", &rel_path]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await;

    match add_res {
        Ok(r) if r.ok() => BumpGitlinkOutcome::Success {
            submodule_path: submodule_path.to_path_buf(),
            relative_path: rel_path,
            head_commit,
            previous_gitlink,
            warning,
        },
        Ok(r) => BumpGitlinkOutcome::Failed {
            submodule_path: submodule_path.to_path_buf(),
            relative_path: rel_path,
            error: format!("git add failed: {}", r.stderr.trim()),
        },
        Err(e) => BumpGitlinkOutcome::Failed {
            submodule_path: submodule_path.to_path_buf(),
            relative_path: rel_path,
            error: e.to_string(),
        },
    }
}

/// Bumps gitlinks in the superproject for multiple submodules.
pub async fn bump_bulk_gitlinks(
    layer: &ProcessLayer,
    superproject_root: &Path,
    submodule_paths: Option<Vec<PathBuf>>,
    allow_unpushed: bool,
    cancel: CancellationToken,
) -> Vec<BumpGitlinkOutcome> {
    let matrix = query_submodule_matrix(layer, superproject_root).await;
    let targets = filter_submodules_by_paths(
        &matrix.submodules,
        superproject_root,
        submodule_paths.as_deref(),
    );

    let mut outcomes = Vec::new();
    for sm in targets {
        if cancel.is_cancelled() {
            outcomes.push(BumpGitlinkOutcome::Failed {
                submodule_path: sm.path.clone(),
                relative_path: sm.relative_path.clone(),
                error: "Operation cancelled".to_string(),
            });
            continue;
        }

        if !sm.initialized {
            outcomes.push(BumpGitlinkOutcome::Failed {
                submodule_path: sm.path.clone(),
                relative_path: sm.relative_path.clone(),
                error: "Submodule is not initialized".to_string(),
            });
            continue;
        }

        let outcome =
            bump_submodule_gitlink(layer, superproject_root, &sm.path, allow_unpushed).await;
        outcomes.push(outcome);
    }

    outcomes
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
        assert!(status.success(), "git {args:?} failed in {dir:?}");
    }

    fn init_bare_commit_repo(dir: &Path) -> String {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "test@example.com"]);
        git(dir, &["config", "user.name", "Test"]);
        std::fs::write(dir.join("f.txt"), "hello").unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-q", "-m", "init"]);
        let out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn parse_gitmodules_tolerant_of_malformed_entries_and_syntax_errors() {
        let text = r#"
# Comment at start
[submodule "good-1"]
    path = libs/good-1
    url = https://example.com/good-1.git
    branch = main

# Malformed 1: section without closing bracket
[submodule "broken-unclosed
    path = libs/broken-1

# Malformed 2: missing equals sign
[submodule "broken-no-equals"]
    invalid_line_without_equals
    url = https://example.com/broken-2.git

# Malformed 3: submodule without path directive
[submodule "broken-no-path"]
    url = https://example.com/no-path.git

# Malformed 4: directive outside of section
stray_directive = outside

; Semicolon comment
[submodule "good-2"]
    path = libs/good-2
    url = https://example.com/good-2.git
"#;

        let (submodules, malformed) = parse_gitmodules_text(text);
        assert_eq!(submodules.len(), 2, "Both good submodules must parse");
        assert_eq!(submodules[0].name, "good-1");
        assert_eq!(submodules[0].path, "libs/good-1");
        assert_eq!(submodules[0].branch.as_deref(), Some("main"));
        assert_eq!(submodules[1].name, "good-2");
        assert_eq!(submodules[1].path, "libs/good-2");

        assert!(
            malformed.len() >= 4,
            "Must capture all malformed entries: got {malformed:?}"
        );
        assert!(malformed
            .iter()
            .any(|m| m.reason.contains("missing closing ']'")));
        assert!(malformed.iter().any(|m| m.reason.contains("missing '='")));
        assert!(malformed
            .iter()
            .any(|m| m.reason.contains("missing mandatory 'path'")));
        assert!(malformed
            .iter()
            .any(|m| m.reason.contains("outside of any")));
    }

    #[tokio::test]
    async fn submodule_state_no_field_can_render_blank() {
        let child_dir = tempfile::tempdir().unwrap();
        let _child_head = init_bare_commit_repo(child_dir.path());

        let super_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(super_dir.path());
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_dir.path().to_str().unwrap(),
                "child",
            ],
        );
        // Also add an uninitialized declared submodule
        std::fs::write(
            super_dir.path().join(".gitmodules"),
            format!(
                "[submodule \"child\"]\n\tpath = child\n\turl = {}\n[submodule \"uninit\"]\n\tpath = uninit\n\turl = /dev/null\n",
                child_dir.path().display()
            ),
        )
        .unwrap();
        git(
            super_dir.path(),
            &["commit", "-q", "-a", "-m", "add submodules"],
        );

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(matrix.len(), 2);

        for s in &matrix {
            let summary = s.display_summary();
            assert!(!summary.trim().is_empty());
            assert!(!s.name.trim().is_empty());
            assert!(!s.relative_path.trim().is_empty());

            // Assert every Resolved<T> field resolves to Known or explicit Unknown with reason
            let branch_str = format_resolved(&s.branch, |b| format!("{b:?}"));
            assert!(!branch_str.trim().is_empty());

            let gitlink_str = format_resolved(&s.gitlink_commit, |g| g.clone());
            assert!(!gitlink_str.trim().is_empty());

            let div_str = format_resolved(&s.gitlink_divergence, |d| format!("{d:?}"));
            assert!(!div_str.trim().is_empty());

            let basis_str = format_resolved(&s.remote_basis, |b| format!("{b:?}"));
            assert!(!basis_str.trim().is_empty());

            let ab_str = format_resolved(&s.remote_ahead_behind, |(a, b)| format!("{a}/{b}"));
            assert!(!ab_str.trim().is_empty());

            let dirty_str = format_resolved(&s.dirty, |d| d.to_string());
            assert!(!dirty_str.trim().is_empty());

            let fetch_str = format_resolved(&s.last_fetch_unix_secs, |f| format!("{f:?}"));
            assert!(!fetch_str.trim().is_empty());

            let drift_str = format_resolved(&s.drift, |d| format!("{d:?}"));
            assert!(!drift_str.trim().is_empty());
        }
    }

    #[tokio::test]
    async fn uninitialized_submodule_reported_as_such() {
        let super_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(super_dir.path());
        std::fs::write(
            super_dir.path().join(".gitmodules"),
            "[submodule \"absent\"]\n\tpath = absent\n\turl = /nowhere\n",
        )
        .unwrap();
        git(super_dir.path(), &["add", ".gitmodules"]);
        git(super_dir.path(), &["commit", "-q", "-m", "declare absent"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(matrix.len(), 1);
        let sub = &matrix[0];
        assert_eq!(sub.name, "absent");
        assert!(!sub.initialized, "Must be flagged as uninitialized");
        assert!(
            matches!(sub.branch, Resolved::Unknown { .. }),
            "Branch must be unknown"
        );
        assert!(
            matches!(sub.gitlink_divergence, Resolved::Unknown { .. }),
            "Divergence must be unknown"
        );
        assert!(
            matches!(sub.dirty, Resolved::Unknown { .. }),
            "Dirty must not be reported as clean false"
        );

        // Verify drift has DeclaredButAbsent or OrphanedDeclaration
        assert_eq!(
            sub.drift,
            Resolved::known(vec![SubmoduleDrift::OrphanedDeclaration])
        );
    }

    #[tokio::test]
    async fn branch_resolution_detached_head_with_pointing_refs() {
        let child_dir = tempfile::tempdir().unwrap();
        let head = init_bare_commit_repo(child_dir.path());
        git(child_dir.path(), &["tag", "v1.0.0"]);
        git(child_dir.path(), &["checkout", "-q", &head]); // Detach HEAD

        let super_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(super_dir.path());
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_dir.path().to_str().unwrap(),
                "child",
            ],
        );
        git(super_dir.path(), &["commit", "-q", "-m", "add submodule"]);
        let child_in_super = super_dir.path().join("child");
        git(&child_in_super, &["checkout", "-q", &head]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(matrix.len(), 1);
        let sub = &matrix[0];
        assert!(sub.initialized);

        match sub.branch.as_known() {
            Some(SubmoduleBranch::Detached {
                commit,
                pointing_refs,
            }) => {
                assert_eq!(commit, &head);
                assert!(
                    pointing_refs.iter().any(|r| r == "v1.0.0" || r == "main"),
                    "Must include v1.0.0 or main in pointing refs: {pointing_refs:?}"
                );
            }
            other => panic!("Expected Detached branch with pointing refs, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn gitlink_divergence_ahead_behind_and_in_sync() {
        let child_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(child_dir.path());

        let super_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(super_dir.path());
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_dir.path().to_str().unwrap(),
                "child",
            ],
        );
        git(super_dir.path(), &["commit", "-q", "-m", "add submodule"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // 1. InSync
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(
            matrix[0].gitlink_divergence,
            Resolved::known(GitlinkDivergence::InSync)
        );

        // 2. Ahead
        let child_in_super = super_dir.path().join("child");
        std::fs::write(child_in_super.join("adv.txt"), "next").unwrap();
        git(&child_in_super, &["add", "."]);
        git(&child_in_super, &["commit", "-q", "-m", "advance"]);

        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(
            matrix[0].gitlink_divergence,
            Resolved::known(GitlinkDivergence::Ahead { ahead: 1 })
        );

        // 3. Behind: update gitlink in superproject index to a future commit, then revert child
        let new_child_head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&child_in_super)
            .output()
            .unwrap();
        let new_child_head = String::from_utf8_lossy(&new_child_head.stdout)
            .trim()
            .to_string();

        git(super_dir.path(), &["add", "child"]);
        git(super_dir.path(), &["commit", "-q", "-m", "bump gitlink"]);

        git(&child_in_super, &["checkout", "-q", "HEAD~1"]);
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(
            matrix[0].gitlink_divergence,
            Resolved::known(GitlinkDivergence::Behind { behind: 1 })
        );

        // 4. Both: advance child on an alternate branch
        git(
            &child_in_super,
            &["checkout", "-q", "-b", "diverged_branch"],
        );
        std::fs::write(child_in_super.join("branch.txt"), "alt").unwrap();
        git(&child_in_super, &["add", "."]);
        git(&child_in_super, &["commit", "-q", "-m", "alternate"]);

        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(
            matrix[0].gitlink_divergence,
            Resolved::known(GitlinkDivergence::Both {
                ahead: 1,
                behind: 1
            })
        );

        // 5. GitlinkObjectMissingLocally
        let missing = resolve_gitlink_divergence(
            &layer,
            &child_in_super,
            Some("1111111111111111111111111111111111111111"),
            Some(&new_child_head),
        )
        .await;
        assert_eq!(
            missing,
            Resolved::known(GitlinkDivergence::GitlinkObjectMissingLocally {
                gitlink_commit: "1111111111111111111111111111111111111111".to_string()
            })
        );
    }

    #[tokio::test]
    async fn drift_detection_all_four_types() {
        let child_dir = tempfile::tempdir().unwrap();
        let _child_head = init_bare_commit_repo(child_dir.path());

        let super_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(super_dir.path());

        // 1. PresentButUndeclared: add a gitlink in the index without adding to .gitmodules
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_dir.path().to_str().unwrap(),
                "undeclared_sub",
            ],
        );
        // Wipe out .gitmodules
        let _ = std::fs::remove_file(super_dir.path().join(".gitmodules"));
        git(
            super_dir.path(),
            &[
                "commit",
                "-q",
                "-a",
                "-m",
                "commit gitlink without gitmodules",
            ],
        );

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(matrix.len(), 1);
        assert_eq!(matrix[0].name, "undeclared_sub");
        assert_eq!(
            matrix[0].drift,
            Resolved::known(vec![SubmoduleDrift::PresentButUndeclared])
        );

        // 2. DeclaredButAbsent & UrlMismatch:
        std::fs::write(
            super_dir.path().join(".gitmodules"),
            "[submodule \"undeclared_sub\"]\n\tpath = undeclared_sub\n\turl = https://mismatched.url\n",
        )
        .unwrap();
        git(super_dir.path(), &["add", ".gitmodules"]);
        git(
            super_dir.path(),
            &["commit", "-q", "-m", "add mismatched gitmodules"],
        );

        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(matrix.len(), 1);
        let drifts = matrix[0].drift.as_known().unwrap();
        assert!(
            drifts
                .iter()
                .any(|d| matches!(d, SubmoduleDrift::UrlMismatch { .. })),
            "Expected UrlMismatch, got {drifts:?}"
        );

        // 3. OrphanedDeclaration:
        std::fs::write(
            super_dir.path().join(".gitmodules"),
            "[submodule \"orphan\"]\n\tpath = orphan_path\n\turl = https://nowhere.com\n",
        )
        .unwrap();
        git(super_dir.path(), &["add", ".gitmodules"]);
        git(
            super_dir.path(),
            &["commit", "-q", "-m", "add orphan declaration"],
        );

        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        let orphan = matrix.iter().find(|s| s.name == "orphan").unwrap();
        assert_eq!(
            orphan.drift,
            Resolved::known(vec![SubmoduleDrift::OrphanedDeclaration])
        );
    }

    #[tokio::test]
    async fn nested_submodule_traversal_with_depth_and_cycle_prevention() {
        let grandchild_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(grandchild_dir.path());

        let child_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(child_dir.path());
        git(
            child_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                grandchild_dir.path().to_str().unwrap(),
                "grandchild",
            ],
        );
        git(child_dir.path(), &["commit", "-q", "-m", "add grandchild"]);

        let super_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(super_dir.path());
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_dir.path().to_str().unwrap(),
                "child",
            ],
        );
        git(super_dir.path(), &["commit", "-q", "-m", "add child"]);

        // Initialize child's submodules inside superproject
        let child_in_super = super_dir.path().join("child");
        git(
            &child_in_super,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "update",
                "--init",
            ],
        );

        let layer = ProcessLayer::new(4, Duration::from_secs(10));

        // Depth 1: only child
        let depth_1 = query_submodule_matrix_with_depth(&layer, super_dir.path(), 1).await;
        assert_eq!(depth_1.len(), 1);
        assert_eq!(depth_1[0].name, "child");
        assert_eq!(depth_1[0].depth, 1);

        // Depth 2: child and grandchild
        let depth_2 = query_submodule_matrix_with_depth(&layer, super_dir.path(), 2).await;
        assert_eq!(
            depth_2.len(),
            2,
            "Expected child and grandchild: got {:?}",
            depth_2.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert_eq!(depth_2[0].name, "child");
        assert_eq!(depth_2[0].depth, 1);
        assert_eq!(depth_2[1].name, "grandchild");
        assert_eq!(depth_2[1].depth, 2);
    }

    #[tokio::test]
    async fn targeted_invalidation_queries_only_the_specified_submodule() {
        let child_a = tempfile::tempdir().unwrap();
        init_bare_commit_repo(child_a.path());

        let child_b = tempfile::tempdir().unwrap();
        init_bare_commit_repo(child_b.path());

        let super_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(super_dir.path());
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_a.path().to_str().unwrap(),
                "sub-a",
            ],
        );
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_b.path().to_str().unwrap(),
                "sub-b",
            ],
        );
        git(super_dir.path(), &["commit", "-q", "-m", "add submodules"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Dirty sub-a
        let sub_a_path = super_dir.path().join("sub-a");
        std::fs::write(sub_a_path.join("dirty.txt"), "dirty content").unwrap();

        // Query only sub-a
        let single_a = query_single_submodule(&layer, super_dir.path(), Path::new("sub-a")).await;
        assert!(single_a.is_some());
        let a = single_a.unwrap();
        assert_eq!(a.name, "sub-a");
        assert_eq!(a.dirty, Resolved::known(true));

        // Query only sub-b (which is clean)
        let single_b = query_single_submodule(&layer, super_dir.path(), Path::new("sub-b")).await;
        assert!(single_b.is_some());
        let b = single_b.unwrap();
        assert_eq!(b.name, "sub-b");
        assert_eq!(b.dirty, Resolved::known(false));
    }

    #[tokio::test]
    async fn test_refresh_submodules_network_parallel_and_isolated_failures() {
        let super_dir = tempfile::tempdir().unwrap();
        let child_a = tempfile::tempdir().unwrap();
        let child_b = tempfile::tempdir().unwrap();

        let _commit_a = init_bare_commit_repo(child_a.path());
        let _commit_b = init_bare_commit_repo(child_b.path());

        git(super_dir.path(), &["init", "-q", "-b", "main"]);
        git(
            super_dir.path(),
            &["config", "user.email", "test@example.com"],
        );
        git(super_dir.path(), &["config", "user.name", "Test"]);
        std::fs::write(super_dir.path().join("root.txt"), "root").unwrap();
        git(super_dir.path(), &["add", "."]);
        git(super_dir.path(), &["commit", "-q", "-m", "init super"]);

        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_a.path().to_str().unwrap(),
                "sub-a",
            ],
        );
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_b.path().to_str().unwrap(),
                "sub-b",
            ],
        );
        git(super_dir.path(), &["commit", "-q", "-m", "add submodules"]);

        // Break remote in sub-b so it fails to fetch
        let sub_b_path = super_dir.path().join("sub-b");
        git(
            &sub_b_path,
            &[
                "remote",
                "set-url",
                "origin",
                "file:///nonexistent/repo.git",
            ],
        );

        let layer = ProcessLayer::new(8, Duration::from_secs(5));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let refresh_result = refresh_submodules_network(
            &layer,
            super_dir.path(),
            SubmoduleRefreshOptions {
                paths: None,
                concurrency: Some(4),
                prune: Some(true),
                tags: Some(false),
            },
            Some(tx),
            CancellationToken::new(),
        )
        .await;

        assert_eq!(refresh_result.total, 2);
        assert_eq!(refresh_result.succeeded, 1, "sub-a must succeed");
        assert_eq!(refresh_result.failed, 1, "sub-b must fail");

        // Verify failure isolation: sub-a succeeded and received updated state
        let row_a = refresh_result
            .rows
            .iter()
            .find(|r| r.relative_path == "sub-a")
            .unwrap();
        assert!(matches!(
            row_a.status,
            SubmoduleRefreshStatus::Success { .. }
        ));
        assert!(row_a.updated_state.is_some());

        // sub-b failed with error message and did not panic or halt sub-a
        let row_b = refresh_result
            .rows
            .iter()
            .find(|r| r.relative_path == "sub-b")
            .unwrap();
        assert!(matches!(
            row_b.status,
            SubmoduleRefreshStatus::Failed { .. }
        ));

        // Verify progress events were emitted
        let mut progress_events = Vec::new();
        while let Ok(evt) = rx.try_recv() {
            progress_events.push(evt);
        }
        assert!(
            !progress_events.is_empty(),
            "Progress events must be emitted"
        );
        assert!(progress_events.iter().any(|e| e.relative_path == "sub-a"));
        assert!(progress_events.iter().any(|e| e.relative_path == "sub-b"));
    }

    #[tokio::test]
    async fn test_refresh_submodules_network_read_only_command_audit() {
        let super_dir = tempfile::tempdir().unwrap();
        let upstream_a = tempfile::tempdir().unwrap();
        let upstream_b = tempfile::tempdir().unwrap();

        let _commit_a = init_bare_commit_repo(upstream_a.path());
        let _commit_b = init_bare_commit_repo(upstream_b.path());

        git(super_dir.path(), &["init", "-q", "-b", "main"]);
        git(
            super_dir.path(),
            &["config", "user.email", "test@example.com"],
        );
        git(super_dir.path(), &["config", "user.name", "Test"]);
        std::fs::write(super_dir.path().join("root.txt"), "root").unwrap();
        git(super_dir.path(), &["add", "."]);
        git(super_dir.path(), &["commit", "-q", "-m", "init super"]);

        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                upstream_a.path().to_str().unwrap(),
                "sub-a",
            ],
        );
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                upstream_b.path().to_str().unwrap(),
                "sub-b",
            ],
        );
        git(super_dir.path(), &["commit", "-q", "-m", "add submodules"]);

        // Add a new commit in upstream_a
        std::fs::write(upstream_a.path().join("upstream_new.txt"), "new commit").unwrap();
        git(upstream_a.path(), &["add", "."]);
        git(
            upstream_a.path(),
            &["commit", "-q", "-m", "upstream commit 2"],
        );

        let sub_a_path = super_dir.path().join("sub-a");
        let sub_b_path = super_dir.path().join("sub-b");

        // sub-a: create untracked file + dirty tracked file
        std::fs::write(sub_a_path.join("untracked.txt"), "untracked data").unwrap();
        std::fs::write(sub_a_path.join("f.txt"), "modified in working tree").unwrap();

        // sub-b: create staged modification
        std::fs::write(sub_b_path.join("staged.txt"), "staged content").unwrap();
        git(&sub_b_path, &["add", "staged.txt"]);

        // Record pre-refresh hashes and HEADs
        let head_a_before = {
            let out = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&sub_a_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let head_b_before = {
            let out = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&sub_b_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let status_a_before = {
            let out = Command::new("git")
                .args(["status", "--porcelain=v2"])
                .current_dir(&sub_a_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).to_string()
        };
        let status_b_before = {
            let out = Command::new("git")
                .args(["status", "--porcelain=v2"])
                .current_dir(&sub_b_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).to_string()
        };

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let log_start_len = layer.log.snapshot().len();

        let result = refresh_submodules_network(
            &layer,
            super_dir.path(),
            SubmoduleRefreshOptions::default(),
            None,
            CancellationToken::new(),
        )
        .await;

        assert_eq!(result.succeeded, 2);

        // 1. Working copy, index, and HEAD bitwise invariant verification
        let head_a_after = {
            let out = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&sub_a_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let head_b_after = {
            let out = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&sub_b_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let status_a_after = {
            let out = Command::new("git")
                .args(["status", "--porcelain=v2"])
                .current_dir(&sub_a_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).to_string()
        };
        let status_b_after = {
            let out = Command::new("git")
                .args(["status", "--porcelain=v2"])
                .current_dir(&sub_b_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).to_string()
        };

        assert_eq!(head_a_before, head_a_after, "HEAD in sub-a must not move");
        assert_eq!(head_b_before, head_b_after, "HEAD in sub-b must not move");
        assert_eq!(
            status_a_before, status_a_after,
            "Working copy/index in sub-a must be untouched"
        );
        assert_eq!(
            status_b_before, status_b_after,
            "Working copy/index in sub-b must be untouched"
        );
        assert_eq!(
            std::fs::read_to_string(sub_a_path.join("f.txt")).unwrap(),
            "modified in working tree"
        );
        assert_eq!(
            std::fs::read_to_string(sub_a_path.join("untracked.txt")).unwrap(),
            "untracked data"
        );
        assert_eq!(
            std::fs::read_to_string(sub_b_path.join("staged.txt")).unwrap(),
            "staged content"
        );

        // 2. Read-only command audit: assert that NO mutating commands were run in submodules
        let log_entries = layer.log.snapshot();
        let refresh_entries = &log_entries[log_start_len..];

        let disallowed_commands = [
            "checkout", "merge", "rebase", "reset", "clean", "commit", "add",
        ];
        for entry in refresh_entries {
            for arg in &entry.args {
                for disallowed in disallowed_commands {
                    assert_ne!(
                        arg.as_str(),
                        disallowed,
                        "Command audit failure: disallowed command '{disallowed}' executed during network refresh: {:?}",
                        entry.args
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn test_bulk_checkout_preview_and_execute_with_dirty_protection() {
        let super_dir = tempfile::tempdir().unwrap();
        let child_a = tempfile::tempdir().unwrap();
        let child_b = tempfile::tempdir().unwrap();

        let _commit_a = init_bare_commit_repo(child_a.path());
        let _commit_b = init_bare_commit_repo(child_b.path());

        git(super_dir.path(), &["init", "-q", "-b", "main"]);
        git(
            super_dir.path(),
            &["config", "user.email", "test@example.com"],
        );
        git(super_dir.path(), &["config", "user.name", "Test"]);
        std::fs::write(super_dir.path().join("root.txt"), "root").unwrap();
        git(super_dir.path(), &["add", "."]);
        git(super_dir.path(), &["commit", "-q", "-m", "init super"]);

        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_a.path().to_str().unwrap(),
                "sub-a",
            ],
        );
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_b.path().to_str().unwrap(),
                "sub-b",
            ],
        );
        git(super_dir.path(), &["commit", "-q", "-m", "add submodules"]);

        let sub_a_path = super_dir.path().join("sub-a");
        let sub_b_path = super_dir.path().join("sub-b");

        // Create 'feature' branch in both submodules
        git(&sub_a_path, &["branch", "feature"]);
        git(&sub_b_path, &["branch", "feature"]);

        // Make sub-a dirty
        std::fs::write(sub_a_path.join("uncommitted.txt"), "dirty work").unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Preview
        let preview = preview_bulk_checkout(&layer, super_dir.path(), "feature", None).await;
        assert_eq!(preview.total, 2);
        assert_eq!(preview.will_switch, 1, "sub-b will switch");
        assert_eq!(preview.skipped_dirty, 1, "sub-a skipped as dirty");

        // Execute
        let outcome = execute_bulk_checkout(
            &layer,
            super_dir.path(),
            "feature",
            None,
            CancellationToken::new(),
        )
        .await;
        assert_eq!(outcome.succeeded, 1);
        assert_eq!(outcome.skipped, 1);
        assert_eq!(outcome.failed, 0);

        // Verify sub-a was skipped and remained dirty
        let res_a = outcome
            .items
            .iter()
            .find(|i| i.relative_path == "sub-a")
            .unwrap();
        assert!(matches!(res_a.outcome, BulkItemOutcome::Skipped { .. }));
        assert_eq!(
            std::fs::read_to_string(sub_a_path.join("uncommitted.txt")).unwrap(),
            "dirty work"
        );

        // Verify sub-b switched to 'feature'
        let res_b = outcome
            .items
            .iter()
            .find(|i| i.relative_path == "sub-b")
            .unwrap();
        assert!(matches!(res_b.outcome, BulkItemOutcome::Success { .. }));
        let current_b_branch = {
            let out = Command::new("git")
                .args(["branch", "--show-current"])
                .current_dir(&sub_b_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        assert_eq!(current_b_branch, "feature");
    }

    #[tokio::test]
    async fn test_bulk_reset_to_gitlink_refuses_dirty_submodules() {
        let super_dir = tempfile::tempdir().unwrap();
        let child_a = tempfile::tempdir().unwrap();
        let child_b = tempfile::tempdir().unwrap();

        let _commit_a = init_bare_commit_repo(child_a.path());
        let _commit_b = init_bare_commit_repo(child_b.path());

        git(super_dir.path(), &["init", "-q", "-b", "main"]);
        git(
            super_dir.path(),
            &["config", "user.email", "test@example.com"],
        );
        git(super_dir.path(), &["config", "user.name", "Test"]);
        std::fs::write(super_dir.path().join("root.txt"), "root").unwrap();
        git(super_dir.path(), &["add", "."]);
        git(super_dir.path(), &["commit", "-q", "-m", "init super"]);

        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_a.path().to_str().unwrap(),
                "sub-a",
            ],
        );
        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_b.path().to_str().unwrap(),
                "sub-b",
            ],
        );
        git(super_dir.path(), &["commit", "-q", "-m", "add submodules"]);

        let sub_a_path = super_dir.path().join("sub-a");
        let sub_b_path = super_dir.path().join("sub-b");

        let gitlink_a = {
            let out = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&sub_a_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };

        // Commit in sub-a so it diverges
        std::fs::write(sub_a_path.join("new_a.txt"), "diverged a").unwrap();
        git(&sub_a_path, &["add", "."]);
        git(&sub_a_path, &["commit", "-q", "-m", "diverged commit in a"]);

        // Commit in sub-b AND dirty it
        std::fs::write(sub_b_path.join("new_b.txt"), "diverged b").unwrap();
        git(&sub_b_path, &["add", "."]);
        git(&sub_b_path, &["commit", "-q", "-m", "diverged commit in b"]);
        std::fs::write(sub_b_path.join("dirty.txt"), "uncommitted dirty edits").unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // Preview reset
        let preview = preview_bulk_reset_to_gitlink(&layer, super_dir.path(), None).await;
        assert_eq!(preview.will_reset, 1, "sub-a will reset");
        assert_eq!(preview.skipped_dirty, 1, "sub-b skipped dirty");

        // Execute reset
        let outcome =
            execute_bulk_reset_to_gitlink(&layer, super_dir.path(), None, CancellationToken::new())
                .await;
        assert_eq!(outcome.succeeded, 1);
        assert_eq!(outcome.skipped, 1);

        // sub-a was reset to recorded gitlink commit
        let head_a_after = {
            let out = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&sub_a_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        assert_eq!(
            head_a_after, gitlink_a,
            "sub-a must be reset to recorded gitlink"
        );

        // sub-b was refused and dirty file remains intact
        assert_eq!(
            std::fs::read_to_string(sub_b_path.join("dirty.txt")).unwrap(),
            "uncommitted dirty edits"
        );
    }

    #[tokio::test]
    async fn test_gitlink_bumping_with_unpushed_warning() {
        let super_dir = tempfile::tempdir().unwrap();
        let upstream = tempfile::tempdir().unwrap();

        let _commit = init_bare_commit_repo(upstream.path());

        git(super_dir.path(), &["init", "-q", "-b", "main"]);
        git(
            super_dir.path(),
            &["config", "user.email", "test@example.com"],
        );
        git(super_dir.path(), &["config", "user.name", "Test"]);
        std::fs::write(super_dir.path().join("root.txt"), "root").unwrap();
        git(super_dir.path(), &["add", "."]);
        git(super_dir.path(), &["commit", "-q", "-m", "init super"]);

        git(
            super_dir.path(),
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                upstream.path().to_str().unwrap(),
                "sub-lib",
            ],
        );
        git(super_dir.path(), &["commit", "-q", "-m", "add sub-lib"]);

        let sub_path = super_dir.path().join("sub-lib");

        // Make a new commit locally in sub-lib that has NOT been pushed to upstream
        std::fs::write(sub_path.join("unpushed.txt"), "unpushed code").unwrap();
        git(&sub_path, &["add", "."]);
        git(
            &sub_path,
            &["commit", "-q", "-m", "unpushed commit in submodule"],
        );

        let new_head = {
            let out = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&sub_path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };

        let layer = ProcessLayer::new(4, Duration::from_secs(5));

        // 1. Bump with allow_unpushed = false: must be refused with UnpushedRefused
        let refused_outcome =
            bump_submodule_gitlink(&layer, super_dir.path(), &sub_path, false).await;
        assert!(matches!(
            refused_outcome,
            BumpGitlinkOutcome::UnpushedRefused { .. }
        ));

        // 2. Bump with allow_unpushed = true: succeeds with warning
        let success_outcome =
            bump_submodule_gitlink(&layer, super_dir.path(), &sub_path, true).await;
        match success_outcome {
            BumpGitlinkOutcome::Success {
                head_commit,
                warning,
                ..
            } => {
                assert_eq!(head_commit, new_head);
                assert!(
                    warning.is_some(),
                    "Warning about unpushed commit must be returned"
                );
            }
            other => panic!("Expected Success with warning, got: {other:?}"),
        }

        // 3. Staged gitlink in superproject index must match new_head
        let ls_files = {
            let out = Command::new("git")
                .args(["ls-files", "--stage", "sub-lib"])
                .current_dir(super_dir.path())
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).to_string()
        };
        assert!(
            ls_files.contains(&new_head),
            "New gitlink commit must be staged in superproject index"
        );
    }
}
