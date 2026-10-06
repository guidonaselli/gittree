# working-copy-and-commits Specification

## Purpose
TBD - created by archiving change build-gittree-client. Update Purpose after archive.

## Requirements

### Requirement: Working copy status
The app SHALL present the working copy as staged, unstaged, untracked and conflicted groups derived from `git status --porcelain=v2`, including rename and copy detection with similarity, submodule modification, type changes, and files that are ignored but explicitly requested. Path handling MUST be byte-exact and MUST correctly render paths containing spaces, quotes, newlines and non-UTF-8 bytes.

#### Scenario: Rename is shown as a rename
- **WHEN** a tracked file is renamed and staged
- **THEN** it appears once as a rename with both paths and the similarity index, not as an unrelated add and delete

#### Scenario: Non-UTF-8 path
- **WHEN** the working tree contains a file whose name is not valid UTF-8
- **THEN** the path renders losslessly in a recoverable form and every action on that row targets the correct file

### Requirement: Staging granularity
The user SHALL be able to stage and unstage whole files, individual hunks, and individual lines including partial-line selections across a contiguous range. Line-level staging MUST be implemented by constructing a patch and applying it with `git apply --cached`, and MUST refuse rather than guess when the patch does not apply cleanly.

#### Scenario: Stage a single line
- **WHEN** the user stages one added line from a hunk containing five changes
- **THEN** exactly that line is staged, the remaining four stay unstaged, and the file appears in both the staged and unstaged groups

#### Scenario: Patch application conflict
- **WHEN** the file changed on disk between rendering the diff and applying a line-level stage
- **THEN** the operation is refused with an explanation, the diff is re-read, and nothing is staged

#### Scenario: Stage across whitespace settings
- **WHEN** whitespace is being visually ignored in the diff view
- **THEN** the constructed patch still reflects the real bytes, and staging does not silently drop whitespace-only changes

### Requirement: Discard is guarded
Discarding changes SHALL name every affected path and state whether the change is recoverable. Discarding unstaged changes to tracked files and deleting untracked files MUST require explicit confirmation, MUST be presented as separate actions, and MUST NOT be bound to a single unmodified keystroke.

#### Scenario: Discard names its blast radius
- **WHEN** the user discards changes with 12 files selected
- **THEN** the confirmation lists the paths, separates tracked reverts from untracked deletions, and states that untracked deletions cannot be recovered by Git

#### Scenario: Discard prefers a recoverable path
- **WHEN** the user discards a large set of tracked changes
- **THEN** the app offers to stash instead, and if the user proceeds with discard the resulting stash or lack of one is stated

### Requirement: Commit
The user SHALL commit staged changes with a message, and MAY amend the last commit, sign off, GPG/SSH sign, and set author. Amend MUST be refused when the commit is already published to a tracked upstream unless the user overrides after being shown the consequence.

#### Scenario: Commit with nothing staged
- **WHEN** the user commits with an empty index and unstaged changes present
- **THEN** the app offers to stage all changes first rather than creating an empty commit, and never stages anything without confirmation

#### Scenario: Amend a pushed commit
- **WHEN** the user amends a commit that exists on the upstream branch
- **THEN** the app states that history will diverge and that a force push will be required, and proceeds only on explicit confirmation

#### Scenario: Hooks are honoured
- **WHEN** a `pre-commit` hook fails
- **THEN** the commit fails, the hook's full output is shown verbatim, and the index is left exactly as it was

#### Scenario: Signing failure is not silent
- **WHEN** commit signing is configured and the signing key is unavailable
- **THEN** the commit fails with the signing error surfaced, and no unsigned commit is created as a fallback

### Requirement: Commit message handling
The commit message editor SHALL preserve the subject/body convention with a visible subject-length guide, honour `commit.template`, honour `core.commentChar`, retain an unsent draft per repository across restarts, and MUST NOT append any tool attribution, signature or generated-by footer.

#### Scenario: Draft survives a restart
- **WHEN** the user types a message, does not commit, and relaunches the app
- **THEN** the draft is restored for that repository

#### Scenario: No attribution is added
- **WHEN** any commit is created through the app
- **THEN** the stored message contains exactly the user's text with no added footer, co-author trailer or tool reference

### Requirement: Ignore management
The user SHALL be able to add a path, its extension, or its directory to `.gitignore` or `.git/info/exclude` from the status list, and SHALL be shown which existing ignore rule matches a given ignored path.

#### Scenario: Explain why a file is ignored
- **WHEN** the user asks why a path is ignored
- **THEN** the app names the source file, line number and pattern that matched, as reported by `git check-ignore -v`
