use git_process::{GitCall, Intent, ProcessLayer};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagEntry {
    pub name: String,
    pub target_commit: String,
    pub target_commit_full: String,
    pub tag_sha: String,
    pub is_annotated: bool,
    pub tagger_name: Option<String>,
    pub tagger_email: Option<String>,
    pub tagger_date: Option<String>,
    pub subject: Option<String>,
    pub message: Option<String>,
    pub commit_subject: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateTagOptions {
    pub name: String,
    pub target_ref: Option<String>,
    pub message: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushTagOptions {
    pub remote: String,
    pub name: String,
    pub force: bool,
}

pub async fn query_tags(layer: &ProcessLayer, root: &Path) -> Result<Vec<TagEntry>, String> {
    let fmt = "%(refname:short)%00%(objectname)%00%(*objectname)%00%(objecttype)%00%(*objecttype)%00%(taggername)%00%(taggeremail)%00%(taggerdate:iso-strict)%00%(contents:subject)%00%(contents)%00%(*contents:subject)%01";
    let res = layer
        .run(
            GitCall::new(
                root,
                [
                    "for-each-ref",
                    &format!("--format={fmt}"),
                    "--sort=-creatordate",
                    "refs/tags",
                ],
            ),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error querying tags: {e}"))?;

    if !res.ok() {
        return Err(format!("git for-each-ref failed: {}", res.stderr.trim()));
    }

    let mut tags = Vec::new();
    let stdout = res.stdout_utf8_lossy();
    let records = stdout.split('\x01');

    for rec in records {
        let trimmed = rec.trim_matches('\n');
        if trimmed.is_empty() {
            continue;
        }

        let fields: Vec<&str> = trimmed.split('\0').collect();
        if fields.len() < 11 {
            continue;
        }

        let name = fields[0].to_string();
        if name.is_empty() {
            continue;
        }

        let object_sha = fields[1].to_string();
        let deref_sha = fields[2].to_string();
        let object_type = fields[3];
        let tagger_name = if fields[5].is_empty() {
            None
        } else {
            Some(fields[5].to_string())
        };
        let tagger_email = if fields[6].is_empty() {
            None
        } else {
            Some(fields[6].trim_matches(|c| c == '<' || c == '>').to_string())
        };
        let tagger_date = if fields[7].is_empty() {
            None
        } else {
            Some(fields[7].to_string())
        };

        let is_annotated = object_type == "tag";
        let (target_commit_full, subject, message, commit_subject) = if is_annotated {
            let target = if deref_sha.is_empty() {
                object_sha.clone()
            } else {
                deref_sha
            };
            let sub = if fields[8].is_empty() {
                None
            } else {
                Some(fields[8].to_string())
            };
            let msg = if fields[9].is_empty() {
                None
            } else {
                Some(fields[9].to_string())
            };
            let c_sub = if fields[10].is_empty() {
                None
            } else {
                Some(fields[10].to_string())
            };
            (target, sub, msg, c_sub)
        } else {
            let c_sub = if fields[8].is_empty() {
                None
            } else {
                Some(fields[8].to_string())
            };
            (object_sha.clone(), None, None, c_sub)
        };

        let target_commit = if target_commit_full.len() >= 7 {
            target_commit_full[..7].to_string()
        } else {
            target_commit_full.clone()
        };

        tags.push(TagEntry {
            name,
            target_commit,
            target_commit_full,
            tag_sha: object_sha,
            is_annotated,
            tagger_name,
            tagger_email,
            tagger_date,
            subject,
            message,
            commit_subject,
        });
    }

    Ok(tags)
}

pub async fn create_tag(
    layer: &ProcessLayer,
    root: &Path,
    opts: CreateTagOptions,
) -> Result<TagEntry, String> {
    let name = opts.name.trim();
    if name.is_empty() {
        return Err("Tag name cannot be empty".to_string());
    }

    // Validate ref format
    let check = layer
        .run(
            GitCall::new(root, ["check-ref-format", &format!("refs/tags/{name}")]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error checking ref format: {e}"))?;

    if !check.ok() {
        return Err(format!("Invalid tag name '{name}'"));
    }

    let mut args = vec!["tag".to_string()];
    if opts.force {
        args.push("-f".to_string());
    }

    if let Some(msg) = opts.message.as_ref().filter(|m| !m.trim().is_empty()) {
        args.push("-a".to_string());
        args.push("-m".to_string());
        args.push(msg.clone());
    }

    args.push(name.to_string());

    if let Some(target) = opts.target_ref.as_ref().filter(|t| !t.trim().is_empty()) {
        args.push(target.clone());
    }

    let res = layer
        .run(
            GitCall::new(root, args),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error creating tag: {e}"))?;

    if !res.ok() {
        return Err(format!("git tag failed: {}", res.stderr.trim()));
    }

    let all_tags = query_tags(layer, root).await?;
    all_tags
        .into_iter()
        .find(|t| t.name == name)
        .ok_or_else(|| format!("Tag '{name}' created but could not be queried"))
}

pub async fn delete_tag(layer: &ProcessLayer, root: &Path, name: &str) -> Result<(), String> {
    let res = layer
        .run(
            GitCall::new(root, ["tag", "-d", name]),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error deleting tag: {e}"))?;

    if !res.ok() {
        return Err(format!("git tag -d failed: {}", res.stderr.trim()));
    }

    Ok(())
}

pub async fn push_tag(
    layer: &ProcessLayer,
    root: &Path,
    opts: PushTagOptions,
) -> Result<(), String> {
    let mut args = vec!["push".to_string(), opts.remote.clone()];
    if opts.force {
        args.push("--force".to_string());
    }
    args.push(format!("refs/tags/{}", opts.name));

    let res = layer
        .run(
            GitCall::new(root, args),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error pushing tag: {e}"))?;

    if !res.ok() {
        return Err(format!("git push tag failed: {}", res.stderr.trim()));
    }

    Ok(())
}

pub async fn delete_remote_tag(
    layer: &ProcessLayer,
    root: &Path,
    remote: &str,
    name: &str,
) -> Result<(), String> {
    let res = layer
        .run(
            GitCall::new(
                root,
                ["push", remote, "--delete", &format!("refs/tags/{name}")],
            ),
            Intent::Write,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error deleting remote tag: {e}"))?;

    if !res.ok() {
        return Err(format!(
            "git push --delete tag failed: {}",
            res.stderr.trim()
        ));
    }

    Ok(())
}

pub async fn query_remote_tags(
    layer: &ProcessLayer,
    root: &Path,
    remote: &str,
) -> Result<Vec<String>, String> {
    let res = layer
        .run(
            GitCall::new(root, ["ls-remote", "--tags", remote]),
            Intent::Read,
            CancellationToken::new(),
        )
        .await
        .map_err(|e| format!("Process error querying remote tags: {e}"))?;

    if !res.ok() {
        return Err(format!("git ls-remote failed: {}", res.stderr.trim()));
    }

    let stdout = res.stdout_utf8_lossy();
    let mut tags = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let refname = parts[1];
            if let Some(tag) = refname.strip_prefix("refs/tags/") {
                if !tag.ends_with("^{}") && !tags.contains(&tag.to_string()) {
                    tags.push(tag.to_string());
                }
            }
        }
    }

    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_process::ProcessLayer;
    use std::time::Duration;
    use tempfile::tempdir;

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_create_query_and_delete_tags() {
        let dir = tempdir().unwrap();
        git(dir.path(), &["init"]);
        git(dir.path(), &["config", "user.name", "Tester"]);
        git(dir.path(), &["config", "user.email", "tester@example.com"]);
        git(
            dir.path(),
            &[
                "commit",
                "--allow-empty",
                "-m",
                "Initial commit for tagging",
            ],
        );

        let layer = ProcessLayer::new(10, Duration::from_secs(30));

        // 1. Create lightweight tag
        let t1 = create_tag(
            &layer,
            dir.path(),
            CreateTagOptions {
                name: "v0.1.0".to_string(),
                target_ref: None,
                message: None,
                force: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(t1.name, "v0.1.0");
        assert!(!t1.is_annotated);
        assert_eq!(
            t1.commit_subject.as_deref(),
            Some("Initial commit for tagging")
        );

        // 2. Create annotated tag
        let t2 = create_tag(
            &layer,
            dir.path(),
            CreateTagOptions {
                name: "v0.2.0".to_string(),
                target_ref: None,
                message: Some("Release 0.2.0\n\nFull description".to_string()),
                force: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(t2.name, "v0.2.0");
        assert!(t2.is_annotated);
        assert_eq!(t2.subject.as_deref(), Some("Release 0.2.0"));
        assert!(t2.message.as_deref().unwrap().contains("Full description"));
        assert_eq!(t2.tagger_name.as_deref(), Some("Tester"));

        // 3. Query all tags
        let all = query_tags(&layer, dir.path()).await.unwrap();
        assert_eq!(all.len(), 2);

        // 4. Delete lightweight tag
        delete_tag(&layer, dir.path(), "v0.1.0").await.unwrap();
        let remaining = query_tags(&layer, dir.path()).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "v0.2.0");

        // 5. Invalid tag name check
        let bad = create_tag(
            &layer,
            dir.path(),
            CreateTagOptions {
                name: "bad tag with spaces".to_string(),
                target_ref: None,
                message: None,
                force: false,
            },
        )
        .await;
        assert!(bad.is_err());
    }

    #[tokio::test]
    async fn test_remote_tag_push_and_delete() {
        let td = tempdir().unwrap();
        let remote_path = td.path().join("remote.git");
        let local_path = td.path().join("local");

        git(
            td.path(),
            &["init", "--bare", remote_path.to_str().unwrap()],
        );
        git(
            td.path(),
            &[
                "clone",
                remote_path.to_str().unwrap(),
                local_path.to_str().unwrap(),
            ],
        );

        git(&local_path, &["config", "user.name", "Tester"]);
        git(&local_path, &["config", "user.email", "tester@example.com"]);
        git(&local_path, &["commit", "--allow-empty", "-m", "Initial"]);
        git(&local_path, &["push", "origin", "HEAD:main"]);

        let layer = ProcessLayer::new(10, Duration::from_secs(30));

        // Create tag
        create_tag(
            &layer,
            &local_path,
            CreateTagOptions {
                name: "v1.0.0".to_string(),
                target_ref: None,
                message: Some("Release 1.0.0".to_string()),
                force: false,
            },
        )
        .await
        .unwrap();

        // Push tag to remote
        push_tag(
            &layer,
            &local_path,
            PushTagOptions {
                remote: "origin".to_string(),
                name: "v1.0.0".to_string(),
                force: false,
            },
        )
        .await
        .unwrap();

        // Query remote tags
        let remotes = query_remote_tags(&layer, &local_path, "origin")
            .await
            .unwrap();
        assert_eq!(remotes, vec!["v1.0.0".to_string()]);

        // Delete remote tag
        delete_remote_tag(&layer, &local_path, "origin", "v1.0.0")
            .await
            .unwrap();

        let remotes_after = query_remote_tags(&layer, &local_path, "origin")
            .await
            .unwrap();
        assert!(remotes_after.is_empty());
    }
}
