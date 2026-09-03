//! Pinned environment so `git` output parsing cannot be changed by the
//! user's shell configuration (locale, pager, aliases, hooks disabled where
//! we explicitly don't want them running, e.g. background reads).

/// Environment variables applied to every invocation, read-only or write.
/// `GIT_PAGER`/`PAGER` empty avoids a pager ever blocking a child process.
/// `LC_ALL=C` pins message and sort locale so parsing is stable across
/// machines. `GIT_TERMINAL_PROMPT=0` on read-only calls stops an interactive
/// credential prompt from hanging headlessly; write calls that legitimately
/// need a prompt override this explicitly at the call site.
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
