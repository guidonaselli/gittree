# repository-workspace Specification

## Purpose
TBD - created by archiving change build-gittree-client. Update Purpose after archive.

## Requirements

### Requirement: Repository discovery and opening
The app SHALL open any directory that `git rev-parse --show-toplevel` resolves, including a worktree, a submodule working directory, and a bare repository opened read-only. Opening MUST NOT write to the repository.

#### Scenario: Open a subdirectory of a repository
- **WHEN** the user opens a path nested inside a working tree
- **THEN** the repository root is resolved and opened, and the originally selected path is revealed in the file tree

#### Scenario: Open a submodule directly
- **WHEN** the user opens a directory that is a submodule working tree
- **THEN** it opens as a first-class repository, and the UI names the superproject it belongs to

#### Scenario: Refuse a non-repository
- **WHEN** the user opens a directory that is not inside a Git repository
- **THEN** the app states that no repository was found at that path and offers to initialize one, without creating anything until confirmed

### Requirement: Repository state model
Every repository SHALL expose a single authoritative state record containing: root path, current branch or detached HEAD with its commit, upstream ref if any, ahead/behind counts against that upstream, counts of staged, unstaged, untracked and conflicted paths, in-progress operation (merge, rebase, cherry-pick, revert, bisect, `am`), stash count, and submodule count. Every UI surface MUST read from that record rather than issuing its own queries.

#### Scenario: Detached HEAD is a first-class state
- **WHEN** HEAD is detached
- **THEN** the state record reports detachment with the resolved commit and any refs pointing at it, and no surface renders an empty or invented branch name

#### Scenario: Interrupted operation is surfaced
- **WHEN** the repository has an in-progress rebase
- **THEN** the app reports the rebase, the step reached, and offers continue, skip and abort before offering unrelated write actions

### Requirement: Workspace bookmarks
The user SHALL be able to bookmark repositories, group them into named folders, reorder them, and reopen the workspace with the same repositories and grouping after a restart. Bookmarks MUST be stored as a plain human-editable file outside any repository.

#### Scenario: Restore workspace after restart
- **WHEN** the app is closed with three bookmarked repositories and one active
- **THEN** relaunching restores all three bookmarks and reactivates the same repository at the same view

#### Scenario: Bookmark to a path that disappeared
- **WHEN** a bookmarked path no longer exists
- **THEN** the bookmark renders as unavailable with its recorded path, and is neither silently deleted nor treated as an error that blocks startup

### Requirement: Filesystem watching and refresh
The app SHALL detect working-tree, index and ref changes made outside the app and refresh affected views without user action. Watching MUST coalesce bursts and MUST NOT rescan the whole repository for a single file change.

#### Scenario: External commit appears
- **WHEN** a commit is made from a terminal in the open repository
- **THEN** the branch, history and status surfaces reflect it without a manual refresh

#### Scenario: Build output churn does not thrash
- **WHEN** a build writes several thousand ignored files in a burst
- **THEN** the app performs at most one coalesced status refresh for that burst, and the UI does not block

#### Scenario: Watching is bounded on huge trees
- **WHEN** the repository exceeds the platform's inotify watch limit
- **THEN** the app degrades to polling on a stated interval and tells the user watching was downgraded and why, rather than silently stopping

### Requirement: Large repository responsiveness
Repository scanning, status and history queries SHALL run off the UI thread. No single synchronous UI block may exceed 100 ms, and any operation exceeding 300 ms MUST show determinate or explicitly indeterminate progress and be cancellable.

#### Scenario: Status of a very dirty tree
- **WHEN** the working tree has 10 000 changed files
- **THEN** the status list begins populating incrementally, remains scrollable while loading, and never blocks the UI thread for more than 100 ms at a time

#### Scenario: Cancel a slow operation
- **WHEN** the user cancels a running scan
- **THEN** the underlying process is terminated, partial results are discarded or clearly marked stale, and the repository is left unmodified

### Requirement: Multiple repositories open concurrently as groups
Opening a repository SHALL create or activate a workspace group for it rather than replacing whatever is currently open. Each group keeps its own state independently of every other group's, and switching the active group MUST NOT discard or refetch state that has not changed.

#### Scenario: Opening a second repository does not close the first
- **WHEN** the user opens a repository while a different one is already open
- **THEN** a new group is created, becomes active, and the previous group's state is retained unchanged

#### Scenario: Switching groups preserves each one's state
- **WHEN** the user switches from group A to group B and back to group A
- **THEN** group A's active tab, scroll position, filters and selection are exactly as they were before switching away

#### Scenario: Closing a group does not affect others
- **WHEN** the user closes one group
- **THEN** every other open group and its tabs remain open and unaffected, and the app activates another open group or the empty state if none remain
