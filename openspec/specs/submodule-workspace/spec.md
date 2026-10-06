# submodule-workspace Specification

## Purpose
TBD - created by archiving change build-gittree-client. Update Purpose after archive.

## Requirements

### Requirement: Submodule state model
Every submodule SHALL resolve to a single state record containing: name and path as declared in `.gitmodules`, configured branch if declared, initialization state, checked-out commit, the commit recorded by the superproject (the gitlink), the branch or detached state of its HEAD, gitlink divergence, remote divergence with its resolution basis, working-copy dirtiness, and any `.gitmodules` drift. No field may be silently absent: each MUST hold either a resolved value or an explicit reason it could not be resolved.

#### Scenario: Every field resolves or explains itself
- **WHEN** the submodule matrix renders for the reference superproject
- **THEN** every row shows a definite value or a definite stated reason for each column, and no cell is blank, dashed or ambiguous

#### Scenario: Uninitialized submodule
- **WHEN** a declared submodule has never been initialized or its directory is empty
- **THEN** the row reports it as uninitialized with its recorded gitlink, offers initialization, and does not report it as clean or in-sync

#### Scenario: Detached submodule
- **WHEN** a submodule's HEAD is detached
- **THEN** the row reports detachment with the commit and any refs pointing at it, and states whether that commit equals the gitlink

### Requirement: Gitlink divergence
For each submodule the app SHALL compute the relationship between its checked-out HEAD and the gitlink the superproject records, expressed as in-sync, or as commit counts behind and ahead in both directions. When the two commits share no reachable ancestry, or the gitlink object is absent locally, that MUST be reported as its own distinct state and never as zero divergence.

#### Scenario: Submodule moved ahead of the superproject
- **WHEN** a submodule's HEAD is 7 commits ahead of the recorded gitlink
- **THEN** the row reports 7 ahead and flags the superproject as needing a gitlink bump

#### Scenario: Superproject records a newer commit
- **WHEN** the gitlink is 80 commits ahead of the submodule's HEAD
- **THEN** the row reports 80 behind and offers to check out the recorded commit

#### Scenario: Gitlink object is missing locally
- **WHEN** the recorded gitlink commit is not present in the submodule's object store
- **THEN** the row states that the recorded commit is unknown locally and offers a fetch, rather than reporting divergence it cannot compute

#### Scenario: Unrelated histories
- **WHEN** HEAD and the gitlink share no common ancestor
- **THEN** the row reports unrelated histories explicitly

### Requirement: Remote divergence with an explicit basis
For each submodule the app SHALL compute ahead/behind against a remote basis resolved by the chain `@{u}` → `<default-remote>/<current-branch>` → unresolved, and MUST display which basis was used. Because upstream tracking is unset on most submodules in real repositories, an unresolved majority is a defect, not an acceptable outcome.

#### Scenario: Inferred basis is labelled
- **WHEN** a submodule on branch `test` has no `@{u}` but `origin/test` exists
- **THEN** divergence is computed against `origin/test` and the basis is shown as inferred

#### Scenario: Staleness is visible
- **WHEN** remote divergence is computed from local remote-tracking refs
- **THEN** the row shows how long ago that submodule was last fetched, so a five-year-old comparison cannot be mistaken for current

#### Scenario: No basis at all
- **WHEN** neither an upstream nor a same-named remote branch exists
- **THEN** the row reports no remote basis and offers to set an upstream

### Requirement: Submodule matrix view
The app SHALL present all submodules of the open superproject as one sortable, filterable table with a row per submodule and columns for name, branch, gitlink divergence, remote divergence, dirtiness and last fetch. The view MUST support selecting an arbitrary subset of rows, filtering by state (dirty, detached, behind gitlink, ahead of gitlink, uninitialized, no remote basis), grouping by branch, and MUST make the count of submodules in each state visible without scrolling.

#### Scenario: Five branch families are legible at once
- **WHEN** 25 submodules sit across five different branch families
- **THEN** grouping by branch shows the five groups with their member counts, and the branch column is never truncated to ambiguity

#### Scenario: Filter to what needs attention
- **WHEN** the user filters to submodules that are dirty or detached
- **THEN** only those rows show, the filter states how many of the total matched, and clearing it restores the full set

#### Scenario: Fits a small viewport
- **WHEN** the window is 1280×720
- **THEN** all six columns remain readable for 25 rows without horizontal scrolling of the identifying columns

### Requirement: Local refresh performance
A full local refresh of every submodule's state, with no network access, SHALL complete within 1.5 s for a superproject of 25 submodules totalling 3.1 GB, and SHALL be recomputed automatically when the superproject index, any submodule HEAD, or any submodule working tree changes.

#### Scenario: Cold open budget
- **WHEN** the reference superproject is opened with a cold app cache and no network
- **THEN** the fully populated matrix is displayed within 1.5 s

#### Scenario: Single submodule change does not rescan everything
- **WHEN** one submodule's HEAD moves
- **THEN** only that submodule's state record is recomputed, and the other rows are not re-queried

### Requirement: Concurrent network refresh
Network refresh across submodules SHALL run concurrently with a user-configurable worker count defaulting to 8, and MUST complete a refresh of 25 submodule remotes within 10 s where the serial equivalent costs approximately 47 s. Each submodule's outcome MUST be reported independently, and one failure MUST NOT abort the others.

#### Scenario: Parallel refresh meets its budget
- **WHEN** the user refreshes all 25 submodules from the network at default concurrency
- **THEN** the refresh completes within 10 s and every row updates its divergence and last-fetch values

#### Scenario: Partial failure is isolated
- **WHEN** 3 of 25 remotes are unreachable
- **THEN** the other 22 update normally, the 3 report their individual errors on their own rows, and the operation is not reported as a global failure

#### Scenario: Progress is per-row and cancellable
- **WHEN** a network refresh is running
- **THEN** each row shows its own in-flight state, the overall progress is a completed count, and cancelling stops pending work without leaving any submodule half-updated

#### Scenario: Refresh does not touch working copies
- **WHEN** a network refresh runs
- **THEN** it only updates remote-tracking refs and objects, and no submodule working tree, HEAD or index is modified

### Requirement: Bulk operations across a selection
The user SHALL apply checkout, fetch, pull, and gitlink-reset to a selected set of submodules in one action. Every bulk operation MUST present a per-submodule preview of what will happen before execution, MUST run with the configured concurrency, MUST report a per-submodule result, and MUST skip rather than force any submodule whose working copy is dirty unless the user explicitly chooses otherwise.

#### Scenario: Bulk checkout of a branch
- **WHEN** the user checks out branch `dev` on 12 selected submodules
- **THEN** the preview lists which will switch, which are already there, and which will be skipped as dirty, and execution reports each outcome individually

#### Scenario: Dirty submodules are protected
- **WHEN** a bulk checkout includes 2 submodules with uncommitted changes
- **THEN** those 2 are skipped by default with the reason stated, and the other rows proceed

#### Scenario: Bulk reset to recorded gitlink
- **WHEN** the user resets a selection to the commits the superproject records
- **THEN** the preview names each target commit, dirty submodules are refused, and after execution those rows report in-sync

#### Scenario: Partial bulk failure is reported honestly
- **WHEN** some submodules in a bulk operation fail
- **THEN** the result names successes, skips and failures separately with per-row reasons, and the summary never reports overall success

### Requirement: Gitlink bump in the superproject
The user SHALL stage updated gitlinks for a selected set of submodules and commit them in the superproject as one commit. The preview MUST show, per submodule, the old and new commit with its subject, and MUST warn when a new commit is not reachable from any remote branch of that submodule because such a commit would be unresolvable for anyone else.

#### Scenario: Bump several submodules in one commit
- **WHEN** the user bumps the gitlinks of 4 submodules
- **THEN** the superproject index stages exactly those 4 gitlinks, the message lists them, and no unrelated path is staged

#### Scenario: Unpushed submodule commit is flagged
- **WHEN** a submodule's new commit exists only locally
- **THEN** the app warns that the gitlink would reference an unreachable commit and offers to push that submodule first

### Requirement: Submodule drill-in as a tab within the superproject's group
The user SHALL open any submodule as a full repository — status, history, diff, staging, branches — from its row, as a new tab within the superproject's own workspace group. Opening a second, third or further submodule MUST NOT close a previously opened submodule tab: multiple submodule tabs stay open concurrently within the same group. Returning to the superproject's own tab MUST NOT lose the matrix's sort, filter or selection state. The UI MUST make clear at all times which tab is active and whether it is the superproject or a specific submodule.

#### Scenario: Round trip preserves context
- **WHEN** the user drills into a submodule from a filtered, sorted matrix and returns to the superproject tab
- **THEN** the same filter, sort and selection are still in effect

#### Scenario: Multiple submodules stay open as separate tabs
- **WHEN** the user drills into `despachomanager` and then, without closing it, drills into `devicepolling`
- **THEN** both appear as separate tabs within the same group, each independently switchable, and neither closes the other

#### Scenario: Reopening an already-open submodule activates its tab
- **WHEN** the user drills into a submodule that already has an open tab in the group
- **THEN** the existing tab is activated rather than a duplicate being created

#### Scenario: Scope is unambiguous
- **WHEN** the user is viewing a submodule tab opened from the matrix
- **THEN** the active repository and its parent superproject are both named in the UI chrome

### Requirement: `.gitmodules` awareness
The app SHALL read `.gitmodules` as the declaration of record and report drift between it, `.git/config`, and the actual working tree — including declared-but-absent submodules, present-but-undeclared directories, URL mismatches, and malformed entries. Malformed entries MUST degrade to a reported error on that entry alone.

#### Scenario: Malformed entry does not break the view
- **WHEN** a `.gitmodules` entry has broken indentation or a trailing-whitespace URL
- **THEN** that entry is reported as malformed with its line number, and all other submodules still render normally

#### Scenario: URL drift is detected
- **WHEN** a submodule's configured URL in `.git/config` differs from `.gitmodules`
- **THEN** the row reports the mismatch with both URLs and offers to sync

#### Scenario: Declared but missing
- **WHEN** `.gitmodules` declares a submodule with no corresponding gitlink in the index
- **THEN** the app reports the declaration as orphaned rather than omitting it

### Requirement: Nested submodules
The app SHALL report submodules nested inside submodules to a configurable depth defaulting to 2, and MUST NOT recurse without bound or hang on a cyclic configuration.

#### Scenario: Cycle does not hang
- **WHEN** submodule configuration forms a cycle
- **THEN** traversal stops at the configured depth, the cycle is reported, and the app stays responsive
