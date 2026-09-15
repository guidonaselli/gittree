use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn run_git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|e| format!("failed to spawn git: {e}"))?;
    if !status.success() {
        return Err(format!("git {args:?} failed in {}", dir.display()));
    }
    Ok(())
}

fn init_commit_repo(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("failed to create dir {}: {e}", dir.display()))?;
    run_git(dir, &["init", "-q", "-b", "main"])?;
    run_git(dir, &["config", "user.email", "fixture@example.com"])?;
    run_git(dir, &["config", "user.name", "Fixture"])?;
    fs::write(dir.join("f.txt"), "seed").map_err(|e| format!("failed to write seed file: {e}"))?;
    run_git(dir, &["add", "."])?;
    run_git(dir, &["commit", "-q", "-m", "seed"])?;
    Ok(())
}

/// Generates a synthetic superproject with `count` submodules.
/// Roughly a third of submodules are advanced past their recorded gitlinks.
pub fn generate_submodule_superproject(root: &Path, count: usize) -> Result<(), String> {
    init_commit_repo(root)?;
    let sources_dir = root.join("sources");
    fs::create_dir_all(&sources_dir).map_err(|e| format!("failed to create sources dir: {e}"))?;

    for i in 0..count {
        let name = format!("sub-{i:02}");
        let child_dir = sources_dir.join(&name);
        init_commit_repo(&child_dir)?;
        run_git(
            root,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                child_dir.to_str().unwrap(),
                &name,
            ],
        )?;
        if i % 3 == 0 {
            let sub_path = root.join(&name);
            fs::write(sub_path.join("g.txt"), "advance")
                .map_err(|e| format!("failed to write advance file: {e}"))?;
            run_git(&sub_path, &["add", "."])?;
            run_git(&sub_path, &["commit", "-q", "-m", "advance past gitlink"])?;
        }
    }
    run_git(root, &["commit", "-q", "-m", "add submodules"])?;
    Ok(())
}

/// Generates `commit_count` commits across `branch_count` branches via `git fast-import`.
pub fn generate_large_history(
    root: &Path,
    commit_count: usize,
    branch_count: usize,
) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| format!("failed to create dir {}: {e}", root.display()))?;
    run_git(root, &["init", "-q", "-b", "main"])?;

    let mut child = Command::new("git")
        .args(["fast-import", "--quiet"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn git fast-import: {e}"))?;

    let num_branches = branch_count.max(1);
    let mut branch_heads: Vec<usize> = vec![0; num_branches];

    {
        let stdin = child.stdin.as_mut().ok_or("failed to open stdin for fast-import")?;
        writeln!(stdin, "blob\nmark :1\ndata 5\nhello")
            .map_err(|e| format!("failed to write initial blob: {e}"))?;

        for i in 0..commit_count {
            let mark = i + 2;
            let b_idx = i % num_branches;
            let branch_name = if b_idx == 0 {
                "main".to_string()
            } else {
                format!("branch-{b_idx}")
            };

            writeln!(stdin, "commit refs/heads/{branch_name}")
                .map_err(|e| format!("failed to write commit ref: {e}"))?;
            writeln!(stdin, "mark :{mark}")
                .map_err(|e| format!("failed to write mark: {e}"))?;
            let timestamp = 1_700_000_000 + i as u64;
            writeln!(stdin, "author Fixture <fixture@example.com> {timestamp} +0000")
                .map_err(|e| format!("failed to write author: {e}"))?;
            writeln!(stdin, "committer Fixture <fixture@example.com> {timestamp} +0000")
                .map_err(|e| format!("failed to write committer: {e}"))?;

            let msg = format!("commit {i} on {branch_name}");
            writeln!(stdin, "data {}\n{msg}", msg.len())
                .map_err(|e| format!("failed to write message: {e}"))?;

            let prev = branch_heads[b_idx];
            if prev > 0 {
                writeln!(stdin, "from :{prev}")
                    .map_err(|e| format!("failed to write parent: {e}"))?;
            } else if b_idx > 0 && branch_heads[0] > 0 {
                // Fork from main branch's current head
                writeln!(stdin, "from :{}", branch_heads[0])
                    .map_err(|e| format!("failed to write fork parent: {e}"))?;
            }
            branch_heads[b_idx] = mark;

            writeln!(stdin, "M 100644 :1 f{}.txt", i % 50)
                .map_err(|e| format!("failed to write file mod: {e}"))?;
        }
    }

    let status = child.wait().map_err(|e| format!("fast-import wait error: {e}"))?;
    if !status.success() {
        return Err("git fast-import exited with failure".to_string());
    }
    // Checkout main branch working tree
    run_git(root, &["reset", "--hard", "HEAD"])?;
    Ok(())
}

/// Generates a dirty tree with `file_count` total dirty files across staged,
/// unstaged, and untracked categories with high performance.
pub fn generate_dirty_tree(root: &Path, file_count: usize) -> Result<(), String> {
    init_commit_repo(root)?;

    // We distribute the requested file_count:
    // - 25% staged modifications (from initially committed files)
    // - 25% staged additions
    // - 25% unstaged modifications (from initially committed files)
    // - 25% untracked files
    let quarter = file_count / 4;
    let initial_committed = quarter * 2; // needed for staged and unstaged mods

    let committed_dir = root.join("committed");
    fs::create_dir_all(&committed_dir)
        .map_err(|e| format!("failed to create committed dir: {e}"))?;

    // Create and commit initial files
    for i in 0..initial_committed {
        let fpath = committed_dir.join(format!("c_{i:06}.txt"));
        fs::write(&fpath, format!("initial content {i}\n"))
            .map_err(|e| format!("write error: {e}"))?;
    }
    run_git(root, &["add", "committed"])?;
    run_git(root, &["commit", "-q", "-m", "initial committed files"])?;

    // 1. Staged modifications: modify first quarter of committed files and stage them
    for i in 0..quarter {
        let fpath = committed_dir.join(format!("c_{i:06}.txt"));
        fs::write(&fpath, format!("staged modified content {i}\n"))
            .map_err(|e| format!("write error: {e}"))?;
    }
    // Stage the modifications
    run_git(root, &["add", "committed"])?;

    // 2. Staged additions: create new files in staged_new and stage them
    let staged_new_dir = root.join("staged_new");
    fs::create_dir_all(&staged_new_dir)
        .map_err(|e| format!("failed to create staged_new dir: {e}"))?;
    for i in 0..quarter {
        let fpath = staged_new_dir.join(format!("s_{i:06}.txt"));
        fs::write(&fpath, format!("staged new content {i}\n"))
            .map_err(|e| format!("write error: {e}"))?;
    }
    run_git(root, &["add", "staged_new"])?;

    // 3. Unstaged modifications: modify the second quarter of committed files (without staging)
    for i in quarter..(quarter * 2) {
        let fpath = committed_dir.join(format!("c_{i:06}.txt"));
        fs::write(&fpath, format!("unstaged modified content {i}\n"))
            .map_err(|e| format!("write error: {e}"))?;
    }

    // 4. Untracked files: remaining files in untracked/
    let untracked_dir = root.join("untracked");
    fs::create_dir_all(&untracked_dir)
        .map_err(|e| format!("failed to create untracked dir: {e}"))?;
    let untracked_count = file_count - (quarter * 3);
    for i in 0..untracked_count {
        let fpath = untracked_dir.join(format!("u_{i:06}.txt"));
        fs::write(&fpath, format!("untracked content {i}\n"))
            .map_err(|e| format!("write error: {e}"))?;
    }

    Ok(())
}
