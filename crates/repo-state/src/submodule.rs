use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

use crate::resolved::Resolved;
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

/// Resilient line-by-line `.gitmodules` parser reporting malformed entries with
/// their exact 1-indexed line numbers while parsing all valid submodule sections.
pub fn parse_gitmodules_text(text: &str) -> (Vec<DeclaredSubmodule>, Vec<MalformedGitmodulesEntry>) {
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

    let finish_submodule = |cur: CurrentSubmodule,
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
                    reason: "Submodule section is missing mandatory 'path' directive".to_string(),
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
                    reason: format!("Unexpected section header: expected [submodule \"<name>\"], got [{inner}]"),
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
            let val = val.trim().trim_matches('"').trim_matches('\'').trim().to_string();

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
async fn read_gitlinks(
    layer: &ProcessLayer,
    superproject_root: &Path,
) -> HashMap<String, String> {
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
async fn resolve_submodule_branch(
    layer: &ProcessLayer,
    path: &Path,
) -> Resolved<SubmoduleBranch> {
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
            GitCall::new(path, ["cat-file", "-e", &format!("{gitlink_sha}^{{commit}}")]),
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

    let gitlink_divergence = resolve_gitlink_divergence(
        layer,
        &path,
        gitlink.as_deref(),
        head_commit.as_deref(),
    )
    .await;

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
        Some(fh) if fh.exists() => {
            match std::fs::metadata(&fh).and_then(|m| m.modified()) {
                Ok(t) => Resolved::known(
                    t.duration_since(SystemTime::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_secs()),
                ),
                Err(e) => Resolved::unknown(e.to_string()),
            }
        }
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

    let (
        (declared_submodules, malformed_entries),
        head_gitlinks,
        index_gitlinks,
        config_urls,
    ) = tokio::join!(
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
        let gitlink = head_gitlinks.get(&path).or_else(|| index_gitlinks.get(&path)).cloned();
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

    let futures = entries_to_query.into_iter().map(|(declared, rel_path, gitlink)| {
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
        cfg.ok().filter(|r| r.ok()).map(|r| r.stdout_utf8_lossy().trim().to_string())
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

        assert!(malformed.len() >= 4, "Must capture all malformed entries: got {malformed:?}");
        assert!(malformed.iter().any(|m| m.reason.contains("missing closing ']'")));
        assert!(malformed.iter().any(|m| m.reason.contains("missing '='")));
        assert!(malformed.iter().any(|m| m.reason.contains("missing mandatory 'path'")));
        assert!(malformed.iter().any(|m| m.reason.contains("outside of any")));
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
        git(super_dir.path(), &["commit", "-q", "-a", "-m", "add submodules"]);

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
        assert!(matches!(sub.branch, Resolved::Unknown { .. }), "Branch must be unknown");
        assert!(matches!(sub.gitlink_divergence, Resolved::Unknown { .. }), "Divergence must be unknown");
        assert!(matches!(sub.dirty, Resolved::Unknown { .. }), "Dirty must not be reported as clean false");

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
            Some(SubmoduleBranch::Detached { commit, pointing_refs }) => {
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
        assert_eq!(matrix[0].gitlink_divergence, Resolved::known(GitlinkDivergence::InSync));

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
        let new_child_head = String::from_utf8_lossy(&new_child_head.stdout).trim().to_string();

        git(super_dir.path(), &["add", "child"]);
        git(super_dir.path(), &["commit", "-q", "-m", "bump gitlink"]);

        git(&child_in_super, &["checkout", "-q", "HEAD~1"]);
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(
            matrix[0].gitlink_divergence,
            Resolved::known(GitlinkDivergence::Behind { behind: 1 })
        );

        // 4. Both: advance child on an alternate branch
        git(&child_in_super, &["checkout", "-q", "-b", "diverged_branch"]);
        std::fs::write(child_in_super.join("branch.txt"), "alt").unwrap();
        git(&child_in_super, &["add", "."]);
        git(&child_in_super, &["commit", "-q", "-m", "alternate"]);

        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(
            matrix[0].gitlink_divergence,
            Resolved::known(GitlinkDivergence::Both { ahead: 1, behind: 1 })
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
        git(super_dir.path(), &["commit", "-q", "-a", "-m", "commit gitlink without gitmodules"]);

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
        git(super_dir.path(), &["commit", "-q", "-m", "add mismatched gitmodules"]);

        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(matrix.len(), 1);
        let drifts = matrix[0].drift.as_known().unwrap();
        assert!(
            drifts.iter().any(|d| matches!(d, SubmoduleDrift::UrlMismatch { .. })),
            "Expected UrlMismatch, got {drifts:?}"
        );

        // 3. OrphanedDeclaration:
        std::fs::write(
            super_dir.path().join(".gitmodules"),
            "[submodule \"orphan\"]\n\tpath = orphan_path\n\turl = https://nowhere.com\n",
        )
        .unwrap();
        git(super_dir.path(), &["add", ".gitmodules"]);
        git(super_dir.path(), &["commit", "-q", "-m", "add orphan declaration"]);

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
            &["-c", "protocol.file.allow=always", "submodule", "update", "--init"],
        );

        let layer = ProcessLayer::new(4, Duration::from_secs(10));

        // Depth 1: only child
        let depth_1 = query_submodule_matrix_with_depth(&layer, super_dir.path(), 1).await;
        assert_eq!(depth_1.len(), 1);
        assert_eq!(depth_1[0].name, "child");
        assert_eq!(depth_1[0].depth, 1);

        // Depth 2: child and grandchild
        let depth_2 = query_submodule_matrix_with_depth(&layer, super_dir.path(), 2).await;
        assert_eq!(depth_2.len(), 2, "Expected child and grandchild: got {:?}", depth_2.iter().map(|s| &s.name).collect::<Vec<_>>());
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
}
