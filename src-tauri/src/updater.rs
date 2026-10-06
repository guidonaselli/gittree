use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: Option<String>,
    pub release_name: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubReleaseResponse {
    tag_name: String,
    html_url: String,
    name: Option<String>,
    published_at: Option<String>,
}

pub async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    let output = tokio::time::timeout(
        Duration::from_secs(6),
        Command::new("curl")
            .args([
                "-s",
                "-f",
                "-H",
                "Accept: application/vnd.github.v3+json",
                "-H",
                "User-Agent: GitTree",
                "--max-time",
                "5",
                "https://api.github.com/repos/guidonaselli/gittree/releases/latest",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| "Request timed out while checking for updates".to_string())?
    .map_err(|e| format!("Failed to execute update check: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            format!("Update check returned status code: {}", output.status)
        } else {
            stderr.trim().to_string()
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let release: GitHubReleaseResponse = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse release information: {e}"))?;

    let latest_tag = release.tag_name.trim_start_matches('v');
    let has_update = is_version_newer(latest_tag, &current_version);

    Ok(UpdateCheckResult {
        current_version: format!("v{current_version}"),
        latest_version: release.tag_name,
        has_update,
        release_url: Some(release.html_url),
        release_name: release.name,
        published_at: release.published_at,
    })
}

fn is_version_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| {
                s.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .collect()
    };
    let l_parts = parse(latest);
    let c_parts = parse(current);
    l_parts > c_parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_version_newer("1.0.1", "1.0.0"));
        assert!(is_version_newer("1.1.0", "1.0.9"));
        assert!(is_version_newer("2.0.0", "1.9.9"));
        assert!(!is_version_newer("1.0.0", "1.0.0"));
        assert!(!is_version_newer("0.9.9", "1.0.0"));
    }
}
