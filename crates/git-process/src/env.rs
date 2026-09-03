//! Pinned environment so `git` output parsing can't be changed by the user's shell config.

/// Environment variables applied to every invocation.
pub fn pinned_env(allow_terminal_prompt: bool) -> Vec<(&'static str, &'static str)> {
    let mut env = vec![
        ("LC_ALL", "C"),
        ("LANG", "C"),
        ("GIT_PAGER", "cat"),
        ("PAGER", "cat"),
        ("GIT_OPTIONAL_LOCKS", "0"),
    ];
    if !allow_terminal_prompt {
        env.push(("GIT_TERMINAL_PROMPT", "0"));
    }
    env
}
