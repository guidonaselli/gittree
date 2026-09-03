use std::path::{Path, PathBuf};
use std::time::SystemTime;

use git_process::{GitCall, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

use crate::resolved::Resolved;
use crate::status::Branch;
use crate::upstream::{resolve_upstream_basis, UpstreamBasis};

/// Divergence between a submodule's checked-out HEAD and the commit the
/// superproject records for it (the gitlink). Spec: submodule-workspace,
/// "Gitlink divergence" — in-sync and zero divergence are not the same
/// state as "we could not compute it".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum GitlinkDivergence {
    InSync,
    Diverged { ahead: u32, behind: u32 },
    GitlinkObjectMissingLocally,
    UnrelatedHistories,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SubmoduleState {
    pub name: String,
    pub path: PathBuf,
    pub declared_branch: Option<String>,
    pub url: Option<String>,
    pub initialized: bool,
    pub gitlink_commit: Resolved<String>,
    pub branch: Resolved<Branch>,
    pub gitlink_divergence: Resolved<GitlinkDivergence>,
    pub remote_basis: Resolved<UpstreamBasis>,
    pub remote_ahead_behind: Resolved<(u32, u32)>,
    pub dirty: Resolved<bool>,
    pub last_fetch_unix_secs: Resolved<Option<u64>>,
}

struct DeclaredSubmodule {
    name: String,
    path: String,
    branch: Option<String>,
    url: Option<String>,
}

/// One call reading `.gitmodules` as the declaration of record (design D3:
/// "two superproject-level calls fan out into per-submodule calls").
async fn read_gitmodules(layer: &ProcessLayer, superproject_root: &Path) -> Vec<DeclaredSubmodule> {
    let result = layer
        .run(
            GitCall::new(
                superproject_root,
                ["config", "-f", ".gitmodules", "--list", "-z"],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;
    let Ok(result) = result else {
        return Vec::new();
    };
    if !result.ok() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&result.stdout);
    let mut by_name: std::collections::BTreeMap<String, DeclaredSubmodule> =
        std::collections::BTreeMap::new();
    for entry in text.split('\0') {
        if entry.is_empty() {
            continue;
        }
        let Some((key, value)) = entry.split_once('\n') else {
            continue;
        };
        // key looks like: submodule.<name>.path / .url / .branch
        let Some(rest) = key.strip_prefix("submodule.") else {
            continue;
        };
        let Some((name, field)) = rest.rsplit_once('.') else {
            continue;
        };
        let entry = by_name
            .entry(name.to_string())
            .or_insert_with(|| DeclaredSubmodule {
                name: name.to_string(),
                path: String::new(),
                branch: None,
                url: None,
            });
        match field {
            "path" => entry.path = value.to_string(),
            "url" => entry.url = Some(value.to_string()),
            "branch" => entry.branch = Some(value.to_string()),
            _ => {}
        }
    }
    by_name
        .into_values()
        .filter(|d| !d.path.is_empty())
        .collect()
}

/// One call reading every gitlink from the superproject's index (design D3).
async fn read_gitlinks(
    layer: &ProcessLayer,
    superproject_root: &Path,
) -> std::collections::HashMap<String, String> {
    let result = layer
        .run(
            GitCall::new(superproject_root, ["ls-tree", "-r", "HEAD", "-z"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;
    let mut map = std::collections::HashMap::new();
    let Ok(result) = result else { return map };
    if !result.ok() {
        return map;
    }
    let text = String::from_utf8_lossy(&result.stdout);
    for line in text.split('\0') {
        if line.is_empty() {
            continue;
        }
        // "<mode> <type> <sha>\t<path>"
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

async fn query_one_submodule(
    layer: &ProcessLayer,
    superproject_root: &Path,
    declared: DeclaredSubmodule,
    gitlink: Option<String>,
) -> SubmoduleState {
    let path = superproject_root.join(&declared.path);
    let git_dir = path.join(".git");
    let initialized = git_dir.exists();

    let gitlink_commit = match &gitlink {
        Some(sha) => Resolved::known(sha.clone()),
        None => Resolved::unknown("no gitlink recorded in the superproject index"),
    };

    if !initialized {
        return SubmoduleState {
            name: declared.name,
            path,
            declared_branch: declared.branch,
            url: declared.url,
            initialized: false,
            gitlink_commit,
            branch: Resolved::unknown("submodule is not initialized"),
            gitlink_divergence: Resolved::unknown("submodule is not initialized"),
            remote_basis: Resolved::unknown("submodule is not initialized"),
            remote_ahead_behind: Resolved::unknown("submodule is not initialized"),
            dirty: Resolved::unknown("submodule is not initialized"),
            last_fetch_unix_secs: Resolved::unknown("submodule is not initialized"),
        };
    }

    let branch = match layer
        .run(
            GitCall::new(&path, ["symbolic-ref", "--short", "-q", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
    {
        Ok(r) if r.ok() => Resolved::known(Branch::Named(r.stdout_utf8_lossy().trim().to_string())),
        _ => match layer
            .run(
                GitCall::new(&path, ["rev-parse", "HEAD"]),
                Intent::Read,
                CancellationToken::new(),
            )
            .await
        {
            Ok(r) if r.ok() => Resolved::known(Branch::Detached {
                commit: r.stdout_utf8_lossy().trim().to_string(),
            }),
            Ok(r) => Resolved::unknown(format!("rev-parse HEAD failed: {}", r.stderr.trim())),
            Err(e) => Resolved::unknown(e.to_string()),
        },
    };
    let branch_name = branch.as_known().and_then(|b| match b {
        Branch::Named(n) => Some(n.clone()),
        Branch::Detached { .. } => None,
    });
    // Always resolved independently of `branch`: needed for gitlink divergence
    // whether HEAD is named or detached.
    let head_commit = layer
        .run(
            GitCall::new(&path, ["rev-parse", "HEAD"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .ok()
        .filter(|r| r.ok())
        .map(|r| r.stdout_utf8_lossy().trim().to_string());

    let gitlink_divergence = match (&gitlink, &head_commit) {
        (Some(gitlink_sha), Some(head_sha)) if gitlink_sha == head_sha => {
            Resolved::known(GitlinkDivergence::InSync)
        }
        (Some(gitlink_sha), Some(head_sha)) => {
            let range = format!("{gitlink_sha}...{head_sha}");
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
                    if behind == 0 && ahead == 0 {
                        // rev-list succeeded but the range is degenerate: no common ancestor edges counted.
                        Resolved::known(GitlinkDivergence::UnrelatedHistories)
                    } else {
                        Resolved::known(GitlinkDivergence::Diverged { ahead, behind })
                    }
                }
                Ok(r)
                    if r.stderr.contains("unknown revision")
                        || r.stderr.contains("bad revision") =>
                {
                    Resolved::known(GitlinkDivergence::GitlinkObjectMissingLocally)
                }
                Ok(r) => Resolved::unknown(format!("rev-list failed: {}", r.stderr.trim())),
                Err(e) => Resolved::unknown(e.to_string()),
            }
        }
        (None, _) => Resolved::unknown("no gitlink recorded in the superproject index"),
        (_, None) => Resolved::unknown("could not resolve submodule HEAD"),
    };

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

    let fetch_head = git_dir.join("FETCH_HEAD");
    let last_fetch_unix_secs = if fetch_head.exists() {
        match std::fs::metadata(&fetch_head).and_then(|m| m.modified()) {
            Ok(t) => Resolved::known(
                t.duration_since(SystemTime::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs()),
            ),
            Err(e) => Resolved::unknown(e.to_string()),
        }
    } else {
        Resolved::known(None)
    };

    SubmoduleState {
        name: declared.name,
        path,
        declared_branch: declared.branch,
        url: declared.url,
        initialized: true,
        gitlink_commit,
        branch,
        gitlink_divergence,
        remote_basis,
        remote_ahead_behind,
        dirty,
        last_fetch_unix_secs,
    }
}

/// Fans out from two superproject-level calls into concurrent per-submodule
/// queries. Concurrency is bounded by the `ProcessLayer`'s own pool, so no
/// separate limiter is needed here (design D5, D3).
pub async fn query_submodule_matrix(
    layer: &ProcessLayer,
    superproject_root: &Path,
) -> Vec<SubmoduleState> {
    let (declared, gitlinks) = tokio::join!(
        read_gitmodules(layer, superproject_root),
        read_gitlinks(layer, superproject_root)
    );

    let futures = declared.into_iter().map(|d| {
        let gitlink = gitlinks.get(&d.path).cloned();
        query_one_submodule(layer, superproject_root, d, gitlink)
    });
    futures::future::join_all(futures).await
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

    #[tokio::test]
    async fn declared_but_missing_submodule_is_reported_as_uninitialized() {
        let super_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(super_dir.path());
        std::fs::write(
            super_dir.path().join(".gitmodules"),
            "[submodule \"child\"]\n\tpath = child\n\turl = /nonexistent\n",
        )
        .unwrap();
        git(super_dir.path(), &["add", ".gitmodules"]);
        git(
            super_dir.path(),
            &["commit", "-q", "-m", "declare submodule"],
        );

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(matrix.len(), 1);
        assert_eq!(matrix[0].name, "child");
        assert!(!matrix[0].initialized);
        assert!(!matrix[0].branch.is_known());
        assert!(!matrix[0].gitlink_divergence.is_known());
    }

    #[tokio::test]
    async fn in_sync_submodule_reports_in_sync() {
        let child_dir = tempfile::tempdir().unwrap();
        let child_head = init_bare_commit_repo(child_dir.path());

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
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(matrix.len(), 1);
        assert!(matrix[0].initialized);
        assert_eq!(matrix[0].gitlink_commit, Resolved::known(child_head));
        assert_eq!(
            matrix[0].gitlink_divergence,
            Resolved::known(GitlinkDivergence::InSync)
        );
        assert_eq!(matrix[0].dirty, Resolved::known(false));
    }

    #[tokio::test]
    async fn submodule_ahead_of_gitlink_is_diverged() {
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

        let child_in_super = super_dir.path().join("child");
        std::fs::write(child_in_super.join("g.txt"), "more").unwrap();
        git(&child_in_super, &["add", "."]);
        git(&child_in_super, &["commit", "-q", "-m", "advance"]);

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        assert_eq!(
            matrix[0].gitlink_divergence,
            Resolved::known(GitlinkDivergence::Diverged {
                ahead: 1,
                behind: 0
            })
        );
    }

    #[tokio::test]
    async fn malformed_gitmodules_entry_does_not_break_the_others() {
        let super_dir = tempfile::tempdir().unwrap();
        init_bare_commit_repo(super_dir.path());
        // One entry with no path (malformed / incomplete) plus one well-formed entry.
        std::fs::write(
            super_dir.path().join(".gitmodules"),
            "[submodule \"broken\"]\n\turl = /nowhere\n[submodule \"good\"]\n\tpath = good\n\turl = /nowhere\n",
        )
        .unwrap();
        git(super_dir.path(), &["add", ".gitmodules"]);
        git(
            super_dir.path(),
            &["commit", "-q", "-m", "declare submodules"],
        );

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let matrix = query_submodule_matrix(&layer, super_dir.path()).await;
        // "broken" has no path so it is filtered out at parse time; "good" still renders.
        assert_eq!(matrix.len(), 1);
        assert_eq!(matrix[0].name, "good");
    }
}
