## ADDED Requirements

### Requirement: Commit history
The app SHALL render commit history for the current branch, all local branches, all refs, or a selected path, with author, date, subject, short hash and ref decorations. History MUST be loaded incrementally and virtualized so that memory and render cost are independent of repository size.

#### Scenario: Very long history
- **WHEN** a repository with 100 000 commits is opened
- **THEN** the first page renders within the interaction budget, scrolling to any point loads on demand without stutter, and memory does not grow proportionally to commit count

#### Scenario: Uncommitted changes row
- **WHEN** the working copy is dirty
- **THEN** history shows an uncommitted-changes entry at the top that selects into the working-copy view

### Requirement: Commit graph
The app SHALL draw the branch and merge topology as a lane graph beside the commit list, with stable lane assignment while scrolling, distinct rendering for merge commits, and legible topology at a minimum of 12 concurrent lanes.

#### Scenario: Lanes do not jump while scrolling
- **WHEN** the user scrolls through a region with many parallel branches and scrolls back
- **THEN** each commit occupies the same lane it did before, and no edge is drawn to an off-screen parent it does not have

#### Scenario: Graph degrades rather than breaks
- **WHEN** topology exceeds the renderable lane budget
- **THEN** overflow lanes are collapsed with an explicit indicator, and the commit list stays fully usable

### Requirement: Commit detail
Selecting a commit SHALL show full hash, author and committer with both dates, full message, parents, refs pointing at it, signature verification status, and its changed files with per-file added and removed counts.

#### Scenario: Merge commit diff basis is stated
- **WHEN** a merge commit is selected
- **THEN** the app states which parent the shown diff is against and lets the user switch parents, rather than presenting an unlabelled combined diff

#### Scenario: Signature status is explicit
- **WHEN** a commit carries a signature
- **THEN** verification reports good, bad, unknown-key or expired distinctly, and never renders an unverified signature as verified

### Requirement: Diff presentation
Diffs SHALL be presentable inline and side-by-side, with syntax highlighting, word-level intra-line change marking, configurable context expansion up to whole file, optional whitespace-change suppression, and configurable tab width. Binary, submodule, symlink, mode-only and very large changes MUST each have an explicit non-textual presentation rather than a blank pane.

#### Scenario: Mode-only change
- **WHEN** a file's only change is its executable bit
- **THEN** the diff states the mode transition explicitly

#### Scenario: Oversized file
- **WHEN** a changed text file exceeds the diff size budget
- **THEN** the app states the size, offers to render anyway, and does not attempt it silently

#### Scenario: Image change
- **WHEN** a changed file is a supported image format
- **THEN** the diff offers a visual before/after comparison

### Requirement: Blame
The app SHALL show per-line blame for any file at any revision, with commit, author and date per line, ability to jump to the blaming commit, ability to reblame from the parent of a given commit to walk past a reformatting change, and optional whitespace-insensitive attribution.

#### Scenario: Walk past a bulk reformat
- **WHEN** every line of a file is attributed to a single formatting commit
- **THEN** the user can reblame from that commit's parent and see the prior attribution

### Requirement: File history
The user SHALL be able to view the commit history of a single file or directory, follow it across renames, diff any two selected revisions of it, and open any historical revision read-only.

#### Scenario: Follow a rename
- **WHEN** the selected file was renamed twice in its past
- **THEN** its history spans all three names, and each rename point is marked

### Requirement: History search
The user SHALL be able to filter history by message text, author, path, date range, and by commit content (`-S`/`-G`). Search MUST be cancellable and MUST report when results are truncated by a limit.

#### Scenario: Content search finds an introduction
- **WHEN** the user searches history for a string that was added and later removed
- **THEN** both the commit that introduced it and the one that removed it are returned

#### Scenario: Truncated results are labelled
- **WHEN** a search exceeds the result limit
- **THEN** the app states that results are truncated and offers to continue searching
