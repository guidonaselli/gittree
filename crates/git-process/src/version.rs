use std::process::Command;

pub const MIN_GIT_VERSION: GitVersion = GitVersion {
    major: 2,
    minor: 43,
    patch: 0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for GitVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn parse_git_version(raw: &str) -> Option<GitVersion> {
    // "git version 2.55.0" (possibly with a distro suffix after the third number).
    let digits = raw
        .split_whitespace()
        .find(|tok| tok.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = digits.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Some(GitVersion {
        major,
        minor,
        patch,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum VersionCheckError {
    #[error("`git` was not found on PATH")]
    NotFound,
    #[error("could not parse `git --version` output: {0:?}")]
    Unparseable(String),
    #[error("found git {found}, GitTree requires at least {required}")]
    TooOld {
        found: GitVersion,
        required: GitVersion,
    },
}

/// Runs synchronously and deliberately: this gates whether the app is allowed
/// to start at all, before any async runtime or worker pool exists.
pub fn check_git_version() -> Result<GitVersion, VersionCheckError> {
    let output = Command::new("git")
        .arg("--version")
        .env("LC_ALL", "C")
        .output()
        .map_err(|_| VersionCheckError::NotFound)?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let found = parse_git_version(&raw).ok_or(VersionCheckError::Unparseable(raw))?;
    if found < MIN_GIT_VERSION {
        return Err(VersionCheckError::TooOld {
            found,
            required: MIN_GIT_VERSION,
        });
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_version_string() {
        assert_eq!(
            parse_git_version("git version 2.55.0"),
            Some(GitVersion {
                major: 2,
                minor: 55,
                patch: 0
            })
        );
    }

    #[test]
    fn parses_distro_suffixed_version_string() {
        assert_eq!(
            parse_git_version("git version 2.43.0.windows.1"),
            Some(GitVersion {
                major: 2,
                minor: 43,
                patch: 0
            })
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_git_version("not a version"), None);
    }

    #[test]
    fn orders_versions_correctly_for_floor_check() {
        assert!(
            GitVersion {
                major: 2,
                minor: 30,
                patch: 0
            } < MIN_GIT_VERSION
        );
        assert!(
            GitVersion {
                major: 2,
                minor: 43,
                patch: 0
            } >= MIN_GIT_VERSION
        );
        assert!(
            GitVersion {
                major: 2,
                minor: 55,
                patch: 0
            } > MIN_GIT_VERSION
        );
    }
}
