## ADDED Requirements

### Requirement: Branch management
The user SHALL create, rename, delete, check out and compare local branches, and SHALL create a local branch tracking a remote branch. Deleting an unmerged branch MUST require explicit confirmation naming the commits that would become unreachable.

#### Scenario: Checkout with uncommitted changes
- **WHEN** the user checks out another branch with conflicting uncommitted changes
- **THEN** the app explains the conflict and offers to stash, commit or cancel, and never discards the changes

#### Scenario: Delete an unmerged branch
- **WHEN** the user deletes a branch holding commits reachable from no other ref
- **THEN** the confirmation states how many commits would be left unreachable and how to recover them from the reflog

### Requirement: Upstream resolution
For any branch, remote divergence SHALL be resolved through an explicit fallback chain: the configured upstream `@{u}` first; failing that, `<default-remote>/<same-branch-name>` if it exists; failing that, an explicit unresolved state. The chosen basis MUST be visible to the user, and MUST NOT be presented as a configured upstream when it was inferred.

#### Scenario: Upstream is not configured but the remote branch exists
- **WHEN** a branch has no `@{u}` but `origin/<branch>` exists locally
- **THEN** ahead/behind is computed against `origin/<branch>` and labelled as inferred, not as tracked

#### Scenario: No basis exists
- **WHEN** a branch has neither an upstream nor a same-named remote branch
- **THEN** the row reports no comparison basis, and no count is invented or shown as zero

### Requirement: Tags and stashes
The user SHALL create lightweight and annotated tags, delete tags, push and delete tags on a remote, and SHALL create, list, inspect, apply, pop and drop stashes including untracked files. Stash inspection MUST show its diff before it is applied.

#### Scenario: Stash then inspect before applying
- **WHEN** the user selects a stash entry
- **THEN** its changed files and diff are shown without modifying the working copy

#### Scenario: Apply a conflicting stash
- **WHEN** applying a stash conflicts with the working copy
- **THEN** conflicts are surfaced for resolution and the stash entry is retained

### Requirement: Integration operations
The user SHALL merge, rebase, cherry-pick and revert, and SHALL be able to continue, skip and abort any of these once in progress. Every one of these operations MUST refuse to start on a dirty working copy unless the user resolves or stashes first.

#### Scenario: Interactive rebase
- **WHEN** the user starts an interactive rebase over a commit range
- **THEN** the plan is editable with pick, reword, edit, squash, fixup and drop, reordering is allowed, and the plan is shown for confirmation before execution

#### Scenario: Abort restores the prior state
- **WHEN** the user aborts a rebase mid-way
- **THEN** HEAD, the branch ref and the working copy return to their pre-rebase state, and the app confirms which state was restored

#### Scenario: Merge commit message is not fabricated
- **WHEN** a merge produces a commit
- **THEN** the user reviews the message before it is written, and no tool attribution is added

### Requirement: Conflict surfacing and resolution
Conflicted paths SHALL be listed distinctly with their conflict type, including both-modified, both-added, delete/modify, rename/rename and submodule conflicts. The user SHALL resolve by choosing ours, theirs, or by launching the configured mergetool, and MUST NOT be able to mark a path resolved while conflict markers remain unless they explicitly override.

#### Scenario: Conflict markers still present
- **WHEN** the user stages a file that still contains conflict markers
- **THEN** the app warns, names the marker lines, and requires explicit confirmation

#### Scenario: Submodule conflict
- **WHEN** a merge conflicts on a submodule gitlink
- **THEN** the app shows both candidate commits with their subjects and lets the user pick one, rather than presenting an unreadable binary conflict

### Requirement: Synchronization
The user SHALL fetch, pull and push, choosing remote, refspec, prune, tags, and rebase-versus-merge for pull, and force-with-lease for push. Plain `--force` push MUST be a separate, explicitly labelled action and MUST NOT be the default.

#### Scenario: Force push prefers a lease
- **WHEN** the user force pushes
- **THEN** `--force-with-lease` is used by default, and a bare force requires a separate explicit choice that states it can destroy others' commits

#### Scenario: Fetch all remotes concurrently
- **WHEN** the repository has multiple remotes and the user fetches all
- **THEN** fetches run concurrently up to the configured limit, each remote's outcome is reported individually, and one failure does not cancel the others

#### Scenario: Push is rejected
- **WHEN** the remote rejects a push as non-fast-forward
- **THEN** the app shows the remote's verbatim message and offers pull-then-retry, without ever force pushing implicitly

### Requirement: Credentials and authentication
The app SHALL authenticate over SSH via the running agent and over HTTPS via the configured Git credential helper. It MUST NOT implement its own credential store, MUST NOT log credentials, and MUST surface interactive prompts — passphrase, host key, 2FA — rather than hanging.

#### Scenario: SSH passphrase is required
- **WHEN** a fetch needs a passphrase for a key not loaded in the agent
- **THEN** the app prompts for it, forwards it to the process, retains it no longer than the operation, and does not write it to disk or logs

#### Scenario: Unknown host key
- **WHEN** the remote host key is not in `known_hosts`
- **THEN** the app shows the fingerprint and requires an explicit decision rather than silently accepting or hanging

#### Scenario: Operation cannot hang forever
- **WHEN** a network operation produces no output for the configured timeout
- **THEN** the app reports it as stalled and offers cancellation, leaving the repository unmodified

### Requirement: Recovery surface
The app SHALL expose the reflog for HEAD and for any branch, showing each entry's operation, commit and message, and SHALL allow resetting a branch to any reflog entry after confirmation. Any operation the app performs that moves a ref MUST be identifiable in that view.

#### Scenario: Undo a bad reset
- **WHEN** the user hard-resets to the wrong commit
- **THEN** the prior position is visible in the reflog view and can be restored in one confirmed action
