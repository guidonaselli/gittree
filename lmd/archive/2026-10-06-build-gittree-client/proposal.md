## Why

Atlassian Sourcetree does not run on Linux and never will: it is closed source and shipped only for macOS and Windows. For a developer whose daily work is a superproject of 25 Git submodules, that is not a cosmetic loss. The single Sourcetree capability that carried the most weight — seeing every submodule at once, with its branch and whether it is ahead of or behind the commit the superproject records — has no equivalent in any Linux client. GitKraken lists submodules in a sidebar. Tower opens them one at a time. `git submodule status` prints a `+` and nothing else. `git submodule foreach` is serial and therefore unusable against the network at this scale.

Measurement on the reference superproject (25 submodules, 3.1 GB) shows the gap is a product gap, not a technical one:

- Reading every submodule's branch, its divergence from the recorded gitlink, and its dirtiness takes **0.32 s** with plain Git plumbing.
- The same repository is, right now, in a state no single command can display: 25 submodules spread across **five simultaneous branch families**, **all 25 divergent** from the recorded gitlink (from 80 behind to 68 ahead), one in detached HEAD, two with uncommitted changes, and one whose last fetch was five years ago.
- The only expensive operation is the network. Querying all 25 remotes serially costs **47 s**; at 8 concurrent workers it costs **7.3 s**. Concurrency is the whole difference between a view that is consulted and one that is avoided.

So the opportunity is twofold. Rebuild the Sourcetree daily loop on Linux — because it is the loop that works and nothing on this platform reproduces it — and then build the submodule workspace that Sourcetree itself only gestured at, on a data model and a concurrency model that Sourcetree never had.

## What Changes

- Establish **GitTree** as the working title: a Tauri 2 desktop Git client for Linux, Rust backend, web frontend, no telemetry, no account, no cloud dependency, fully usable offline.
- Take Git data from the **system `git` CLI** through a bounded async worker pool, not from libgit2 or gitoxide. `git` is the correctness reference for free, and `gix` has documented gaps precisely in submodule status.
- Deliver **Sourcetree parity on the core loop first**: repository workspace, file status, hunk- and line-level staging, commit and amend, history graph, diff and blame, branches, tags, stashes, merge/rebase/cherry-pick, fetch/pull/push, and conflict resolution.
- Then deliver the **submodule workspace**, the differentiator: one always-current matrix of every submodule showing branch, divergence from the recorded gitlink, divergence from its remote, dirtiness and detached state; parallel refresh; bulk checkout, fetch and pull across a selection; and one-action gitlink bumps in the superproject.
- Resolve remote divergence through an explicit **`@{u}` → `origin/<branch>` fallback chain**, because upstream tracking is unset on most submodules in real repositories and a view that reports "unknown" for the majority of rows is worthless.
- Ship a **first-class theme system**: every colour, spacing and type value resolves through design tokens with no hardcoded values anywhere in the UI, a built-in light and dark theme, user-authored themes as plain files, and optional adoption of the desktop's active Omarchy theme.
- Ship **keyboard-first operation**: a command palette, and no action reachable only by mouse.
- Package for Linux as AppImage and an Arch package, with Flatpak as a stretch target.
- Use **OpenSpec** as the canonical requirements and design layer and **LMD** under `05_PLANNING/` as the progressive execution tracker.

### Measurable product outcomes

- Opening the 25-submodule / 3.1 GB reference superproject to a fully populated submodule matrix takes **≤ 1.5 s** with no network access.
- A full network refresh of all 25 submodule remotes completes in **≤ 10 s** at the default concurrency, against 47 s for the serial equivalent.
- The status of a repository with 10 000 changed files renders without the UI thread blocking for more than **100 ms** at a time.
- History renders **100 000 commits** with constant memory and no visible scroll stutter.
- Every submodule row resolves a definite branch state and a definite divergence state — including detached HEAD and missing upstream — or states explicitly why it cannot, with **zero silent blanks**.
- Every destructive action names the exact refs and files it will affect, and every one that Git cannot undo is recoverable from the app's own reflog view or is refused.
- **Every** action in the app is reachable from the keyboard, and the app is operable at 1280×720.
- Idle memory stays under **250 MB** with the reference superproject open; the packaged binary stays under **40 MB**.
- The app performs **no network request** that the user did not initiate, and ships no telemetry, analytics, crash reporting or update check that transmits data.

### Non-goals

- Not a Sourcetree clone at the pixel or menu level. Parity is on capability and workflow; the interaction model is free to be better, and where Sourcetree is bad — its submodule handling, its performance on large repositories, its modal dialogs — it is explicitly not the reference.
- No Git reimplementation. If `git` can do it, GitTree calls `git`.
- No hosting-provider integration in 1.0: no pull request, issue, pipeline or code review surfaces. Those belong to the forge CLIs.
- No Git LFS, no Mercurial, no Perforce, no SVN bridge in 1.0.
- No macOS or Windows build in 1.0. The architecture must not actively prevent one, but no effort is spent enabling it.
- No merge conflict resolution editor of its own in 1.0: conflicts are surfaced, staged and resolved, but three-way editing delegates to the user's configured mergetool.
- No accounts, no sync, no licensing, no paid tier, no telemetry — ever, not just in 1.0.

## Capabilities

### New Capabilities

- `repository-workspace`: Opening, scanning, bookmarking and switching repositories; the repository state model; filesystem watching, refresh and invalidation; multi-repository navigation.
- `working-copy-and-commits`: Working tree and index status; staging and unstaging by file, hunk and line; discard; commit, amend and message handling; ignore and exclude management.
- `history-and-diff`: Commit log and graph rendering; commit detail; the diff engine and its inline, side-by-side and word-level presentations; blame; file history; history search.
- `branches-remotes-sync`: Branches, tags and stashes; checkout; merge, rebase and cherry-pick; remotes; fetch, pull and push; conflict surfacing and resolution; credentials and SSH agent integration; the reflog and recovery surface.
- `submodule-workspace`: The submodule matrix and its state model; gitlink and remote divergence; parallel refresh; bulk and per-submodule operations; superproject gitlink bumps; `.gitmodules` awareness and drift detection.
- `ui-shell-and-theming`: Application layout and navigation; the design-token architecture; built-in and user themes; desktop theme adoption; the command palette; keyboard operability and accessibility.
- `platform-and-packaging`: Linux packaging and distribution; settings persistence; credential handling; performance budgets and their enforcement; the offline and no-telemetry guarantees.

### Modified Capabilities

<!-- Greenfield project; no existing capabilities. -->

## Impact

- Creates a new Rust workspace with a Tauri 2 application shell, a Git process layer, a repository state layer, a web frontend, an integration test suite backed by fixture repositories, and Linux packaging.
- Establishes the submodule state model — branch, gitlink divergence, remote divergence, dirtiness, detachment, `.gitmodules` drift — as the product's central data contract, and the concurrency model that keeps it cheap to compute.
- Pins the reference superproject as a permanent performance fixture: every performance budget in this change is measured against a synthetic equivalent of 25 submodules and 3.1 GB, committed as a generator rather than as data.
- Introduces a persistent LMD initiative mapping milestones, slices, tasks, gates and evidence back to these capabilities.
- Accepts one architectural risk explicitly: WebKitGTK is the only WebView available to Tauri on Linux and has known performance limits. The commit graph is the one surface where that can bite, and it carries a mandatory spike and a documented fallback before its milestone starts.
