use git_process::{GitCall, GitError, Intent, ProcessLayer};
use tokio_util::sync::CancellationToken;

/// The basis used to compute remote divergence: a configured `@{u}`, an
/// inferred `<remote>/<branch>`, or none.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UpstreamBasis {
    Configured { refname: String },
    Inferred { refname: String },
    None,
}

impl UpstreamBasis {
    pub fn refname(&self) -> Option<&str> {
        match self {
            UpstreamBasis::Configured { refname } | UpstreamBasis::Inferred { refname } => {
                Some(refname)
            }
            UpstreamBasis::None => None,
        }
    }
}

/// `@{u}` -> `<default-remote>/<branch>` -> `None`.
pub async fn resolve_upstream_basis(
    layer: &ProcessLayer,
    repo_root: &std::path::Path,
    current_branch: Option<&str>,
) -> UpstreamBasis {
    let configured = layer
        .run(
            GitCall::new(repo_root, ["rev-parse", "--abbrev-ref", "@{u}"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;
    if let Ok(result) = configured {
        if result.ok() {
            let refname = result.stdout_utf8_lossy().trim().to_string();
            if !refname.is_empty() {
                return UpstreamBasis::Configured { refname };
            }
        }
    }

    let Some(branch) = current_branch else {
        return UpstreamBasis::None;
    };

    let default_remote = default_remote_name(layer, repo_root)
        .await
        .unwrap_or_else(|| "origin".to_string());
    let candidate = format!("{default_remote}/{branch}");
    let exists = layer
        .run(
            GitCall::new(repo_root, ["rev-parse", "--verify", "--quiet", &candidate]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map(|r| r.ok())
        .unwrap_or(false);

    if exists {
        UpstreamBasis::Inferred { refname: candidate }
    } else {
        UpstreamBasis::None
    }
}

async fn default_remote_name(layer: &ProcessLayer, repo_root: &std::path::Path) -> Option<String> {
    let result: Result<_, GitError> = layer
        .run(
            GitCall::new(repo_root, ["remote"]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await;
    let result = result.ok()?;
    if !result.ok() {
        return None;
    }
    let names: Vec<String> = result
        .stdout_utf8_lossy()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if names.iter().any(|n| n == "origin") {
        Some("origin".to_string())
    } else {
        names.into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;
    use tempfile::TempDir;

    fn init_repo() -> TempDir {
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
        std::fs::write(dir.path().join("f.txt"), "hello").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        dir
    }

    #[tokio::test]
    async fn no_remote_and_no_upstream_resolves_to_none() {
        let dir = init_repo();
        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let basis = resolve_upstream_basis(&layer, dir.path(), Some("main")).await;
        assert_eq!(basis, UpstreamBasis::None);
    }

    #[tokio::test]
    async fn origin_branch_without_at_u_resolves_as_inferred() {
        let dir = init_repo();
        // Simulate a remote-tracking ref existing locally without @{u} configured,
        // which is exactly the state measured on the reference superproject.
        let head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
        std::fs::create_dir_all(dir.path().join(".git/refs/remotes/origin")).unwrap();
        std::fs::write(
            dir.path().join(".git/refs/remotes/origin/main"),
            format!("{head}\n"),
        )
        .unwrap();

        let layer = ProcessLayer::new(4, Duration::from_secs(5));
        let basis = resolve_upstream_basis(&layer, dir.path(), Some("main")).await;
        assert_eq!(
            basis,
            UpstreamBasis::Inferred {
                refname: "origin/main".to_string()
            }
        );
    }
}
