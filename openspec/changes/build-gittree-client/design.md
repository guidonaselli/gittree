# Design — GitTree

## Context

Sourcetree does not exist on Linux. The capability that mattered most in it — one view of every submodule with its branch and its divergence from the superproject — is absent from every Linux client, and absent from Git's own porcelain. The reference workload is a superproject of 25 submodules totalling 3.1 GB, and its current real state is the design brief:

| Signal | Measured value |
|---|---|
| Submodules | 25 |
| Repository size | 3.1 GB |
| Simultaneous branch families | 5 (`test`, `dev`, `feat/kpi-despachos`, `chore/quitar-toggle-…`, `fix/reenvio-…`) |
| Submodules diverging from their gitlink | 25 of 25 (range: 80 behind → 68 ahead) |
| Detached HEAD | 1 (`common-security`) |
| Dirty working copies | 2 (`dvrmanager`, `keycloak-manager`) |
| Submodules with `@{u}` configured | minority; `origin/<branch>` present for all sampled |
| Oldest last-fetch | ~5 years (`plataformamanager`) |

Two measurements set the architecture:

- **Local state is cheap.** Branch, gitlink divergence and dirtiness for all 25 submodules computes in **0.32 s** with plumbing. `git submodule status` alone is 0.11 s. There is no need for a cache layer, an index, or a background daemon to make the core view fast.
- **Network is the only cost, and it parallelizes.** `ls-remote` across the 25 remotes costs **47 s serially** and **7.3 s at 8 workers**. Concurrency is not an optimization; it is the feature. `git submodule foreach` is serial, which is precisely why the CLI cannot deliver this view in practice.

## Goals / Non-Goals

**Goals**
- Sourcetree's daily loop, on Linux, with better performance and a better interaction model.
- A submodule workspace with no equivalent on any platform: every submodule's branch, gitlink divergence, remote divergence and dirtiness, always current, refreshable in seconds.
- Theming as architecture, not decoration: no hardcoded visual value anywhere.
- Fully local, fully offline-capable, no account, no telemetry, ever.

**Non-Goals**
- Reimplementing Git.
- Copying Sourcetree's UI, dialogs or menu structure.
- Forge integration (PRs, issues, pipelines) in 1.0.
- macOS/Windows builds in 1.0.
- A built-in three-way merge editor in 1.0.

## Decisions

### D1 — Git access shells out to the `git` CLI, not libgit2 or gitoxide

**Decision.** All Git data and all Git writes go through invocations of the system `git` binary (minimum 2.43), inside a single process layer.

**Why.**
1. *It is already fast enough.* 0.32 s for the full 25-submodule sweep, measured. The performance argument for an in-process library does not apply to this workload.
2. *Correctness is free.* Submodules, worktrees, `.gitmodules` parsing, credential helpers, hooks, signing, `include`/`includeIf` config, and every edge case of `git apply` behave exactly as the user's own Git does. A library reimplements all of that, and every divergence is a bug the user experiences as the app lying.
3. *`gix` is weakest exactly where this product is strongest.* Gitoxide's submodule support has documented gaps against `git2`; `Submodule::state()` explicitly does not perform a worktree status. Building the differentiator on the least mature part of a young library inverts the risk.
4. *Hooks and config must work.* Users have `pre-commit` hooks, signing keys and credential helpers. Only the real binary honours all of it.

**Rejected alternatives.**
- *`git2` (libgit2 bindings).* In-process and mature, but its submodule status is still not `git`'s, it does not run hooks, its credential handling diverges from the credential-helper ecosystem, and it adds a C dependency and build complexity for a workload that is not CPU-bound.
- *`gix`.* Fast and pure Rust, and worth revisiting for read-only hot paths later, but the submodule gaps make it disqualifying for 1.0's core.
- *Hybrid from day one.* Two Git implementations means two sets of semantics to reconcile. Not before there is a measured hot path that needs it.

**Consequences.** Process spawn cost (~1–3 ms) is the unit cost, so the design must batch: prefer one `git` call over N, and parallelize what cannot be batched. Output parsing becomes a correctness surface and needs its own tests. `git` version differences must be pinned by a floor and covered by fixtures.

### D2 — Tauri 2 shell, Rust backend, web frontend

**Decision.** Tauri 2 with a Rust backend and a web frontend. The reference machine already has `webkit2gtk-4.1 2.52.5`, `rust 1.96.1` and `node 26.1`.

**Why.** Theming is a stated product requirement, and CSS custom properties make a token architecture and live theme switching nearly free — this is the single largest lever on "better UI with themes". Rust gives a natural home for the process pool, cancellation and the state model. The binary and memory budgets (< 40 MB, < 250 MB idle) are achievable, which matters for an app that stays open all day.

**Rejected alternatives.**
- *GTK4 + libadwaita.* Native and integrates with the system theme, but user-authored custom themes are far more rigid than CSS, and the widget ecosystem for dense virtualized tables and a commit graph is thinner. Theming is a headline requirement; this trades it away.
- *Electron.* Best rendering predictability and the strongest UI/graph ecosystem, but ~150 MB and high resident memory for an always-open tool. Rejected on budget, not capability.
- *TUI (ratatui).* Cheapest and fastest, but "better UI and themes" is the request; a TUI answers a different question. `lazygit` already occupies that niche and is installed on the reference machine.

**Consequences and accepted risk.** WebKitGTK is the only Linux WebView available to Tauri and has known performance limits. The primary surfaces are virtualized tables and text, which are well within it. The commit graph is the one surface that can genuinely suffer. **Mitigation is mandatory and gated:** a rendering spike precedes the history milestone, comparing DOM, 2D canvas and WebGL lane rendering against the 100 000-commit fixture, with a documented fallback (canvas, then reduced lane budget) chosen on evidence before that milestone starts. See R1.

### D3 — Submodule state is one record, computed by a fixed pipeline

**Decision.** Every submodule resolves to a single `SubmoduleState` record. Every UI surface reads that record; no surface issues its own Git query.

Fields, and how each is obtained:

| Field | Source |
|---|---|
| `name`, `path`, `declared_branch`, `url` | `git config -f .gitmodules --get-regexp` (one call, superproject) |
| `gitlink_commit` | `git ls-tree -r HEAD` filtered to mode `160000` (one call, superproject) |
| `initialized` | presence of the submodule's git dir |
| `head` | `git -C <path> rev-parse HEAD` |
| `branch_state` | `git -C <path> symbolic-ref --short HEAD`, else detached with refs pointing at HEAD |
| `gitlink_divergence` | `git -C <path> rev-list --left-right --count <gitlink>...<head>` |
| `remote_basis` | resolution chain, see D4 |
| `remote_divergence` | `rev-list --left-right --count <basis>...<head>` |
| `dirty` | `git -C <path> status --porcelain=v2 -z` |
| `last_fetch` | mtime of `FETCH_HEAD` |
| `gitmodules_drift` | `.gitmodules` vs `.git/config` vs index |

Two superproject-level calls fan out into per-submodule calls that run concurrently in the pool. Every field is a `Resolved(T) | Unresolved(reason)` — never an implicit blank, zero or dash. The spec makes silent blanks a defect, and the type system is what enforces it.

**Why one record.** The failure mode of every existing tool here is inconsistency: the sidebar says one thing, the status bar another, and a refresh in one pane does not update the other. A single record with a single invalidation path removes that class of bug structurally.

### D4 — Remote basis is an explicit fallback chain, and its choice is visible

**Decision.** Remote divergence resolves through `@{u}` → `<default-remote>/<current-branch>` → `Unresolved`. The basis actually used is part of the state record and is displayed.

**Why.** Measured: `@{u}` is unset on most submodules in the reference repository, while `origin/<branch>` exists for all sampled. A tool that only consults `@{u}` reports "unknown" for the majority of rows and is therefore useless. Equally, presenting an inferred basis as a configured upstream is a lie the user will eventually act on. Both the fallback and the labelling are required.

**Consequence.** Divergence computed from local remote-tracking refs is only as fresh as the last fetch — and one submodule's last fetch was five years ago. `last_fetch` is therefore not a nice-to-have column; it is what stops a stale comparison from being read as current.

### D5 — Concurrency is a bounded pool; writes serialize per repository

**Decision.** One bounded async worker pool, default 8, user-configurable. Read operations run freely up to the limit. Write operations against the same repository serialize. Every operation carries a timeout, a cancellation token, and kills its child process on cancel or exit.

**Why.** 47 s → 7.3 s is the whole product. But unbounded fan-out over 25 SSH connections invites remote-side throttling and local resource exhaustion, and concurrent writes against one index corrupt it. Per-repository write serialization keeps submodules — which are independent repositories — genuinely parallel for writes, while making a single repository safe.

**Consequence.** Bulk operations report per-submodule outcomes, never a single aggregate verdict. Partial failure is the normal case at this scale — 3 of 25 remotes unreachable must not read as failure, and 22 of 25 succeeding must not read as success.

### D6 — Refresh is read-only, by construction

**Decision.** No automatic path — watcher, refresh, scan, navigation — may execute a Git command that writes. The single exception is that a user-initiated fetch updates remote-tracking refs and objects. Read-only-ness is enforced by an allowlist in the process layer and asserted by a test that audits executed commands.

**Why.** This is the trust foundation. A tool that touches 25 repositories in the background is one bug away from being uninstallable. Making it a structural property with a test, rather than a convention, is the only version of this that holds.

### D7 — Token-only styling, enforced by lint

**Decision.** Every visual value resolves through a named design token. A build-time lint fails on any literal colour or raw spacing value in a component.

**Why.** "Themes" degrade into "one theme plus exceptions" the moment a single hardcoded colour ships. The lint is what makes user themes actually complete, and it is cheap now and expensive later. Missing tokens in a user theme fall back per-token to the built-in of matching polarity, so a partial theme renders correctly rather than unstyled.

### D8 — Fixtures are generated, not committed

**Decision.** Performance fixtures — a 25-submodule superproject, a 100 000-commit history, a 10 000-file dirty tree — are produced by a committed generator script. No large binary fixture enters the repository.

**Why.** The budgets in these specs are only real if CI measures them. Committing multi-gigabyte fixtures to assert that is self-defeating.

### D9 — One workspace group per open repository, tab strip scoped to the active group

**Decision.** Opening a repository creates a *workspace group* — its own tab strip, its own active tab, its own state — rather than replacing whatever else is open. A group switcher picks which group's tab strip is visible; other groups' state stays alive in memory but hidden. A submodule opened from the matrix becomes a tab inside its superproject's group, not a new top-level view; opening an already-open submodule activates its existing tab instead of duplicating it.

**Why.** The daily shape of this tool's use is several submodule-heavy superprojects worked in parallel, each with a few submodules drilled into at once. A single `activeRoot` — what F-001 shipped first — makes opening a second repository destroy the first one's context, which is exactly the friction Sourcetree and every competing client share. Scoping the tab strip to one group at a time (rather than flattening every group's tabs into one strip) keeps the UI legible as the number of open repositories grows; the group switcher is the one extra click that buys that legibility back.

**Rejected alternatives.**
- *One flat tab strip for everything.* Simplest data model, but a tab strip mixing tabs from unrelated superprojects stops being scannable past two or three open repositories — exactly the scale this tool exists for.
- *Sidebar tree instead of tabs.* Avoids a group switcher, but loses the tab strip's quick-glance "what's open right now" and its keyboard-cycle ergonomics; also a bigger rework of the existing sidebar-as-bookmarks-list design already shipped in F-001.

**Consequences.** The single `activeRoot` signal becomes a list of groups plus an active group id; each group owns its own tab list, active tab, and — for the submodule matrix specifically — its own filter/sort/selection state, matching the existing "drill-in preserves matrix state" requirement one level up. Group and tab switching must stay a pure UI operation with no refetch of unchanged data, which the existing per-resource `createResource` caching already gives for free as long as switching doesn't unmount the underlying resources.

### D10 — Desktop theme sync reads a published palette file, live, generically labelled

**Decision.** Where the desktop publishes its active theme as a stable, readable file (on this reference environment: `~/.local/state/omarchy/current/theme/colors.toml`, a flat TOML file with `mode`, an accent, four background levels, three foreground levels, and ANSI-style semantic colours), GitTree reads it, maps its roles onto GitTree's own design tokens by a fixed table, and re-applies them live on change — reusing the same filesystem-watcher infrastructure built for repository state (D-adjacent to the F-001 watcher, not a new subsystem). The mechanism is real and specific in code; the UI never names the source desktop or theme daemon, describing it only as following the desktop's theme.

**Why.** A generic `prefers-color-scheme` media query only ever gives light-or-dark; it cannot express an accent colour or the desktop's actual palette. Reading the published file gives the same fidelity of sync as the reference IntelliJ/desktop integrations already in daily use, at effectively zero new infrastructure cost, since the debounced watcher already exists. Genericizing the UI copy keeps the feature (and the token-mapping table) portable to any desktop that publishes a similar file later, and avoids coupling GitTree's identity to one desktop environment's branding.

**Rejected alternatives.**
- *Poll instead of watch.* Rejected for the same reason polling is already the documented fallback, not the default, for repository watching: it is strictly worse than an event when an event is available.
- *Hardcode the desktop name in settings ("Omarchy sync").* Rejected per explicit product direction — the feature must read as "follow my desktop," not advertise a specific vendor.

**Consequences.** This sits behind the existing "explicit choice wins" precedence already specified for desktop theme adoption: an explicit user theme always overrides sync, and sync itself is one step below that, above the plain light/dark default. A malformed or missing palette file must fall back to the built-in theme exactly as a missing `prefers-color-scheme` signal already does — no new failure mode, just a richer success case.

## Data contracts

- **Process layer → state layer.** Every invocation returns exit status, stdout, stderr and duration. Parsers consume `-z`-delimited or `--porcelain=v2` output only. A non-zero exit is never converted into a success with partial data.
- **State layer → UI.** The UI receives whole state records and diffs of them. It never receives raw Git output to parse, and it never issues a Git command directly.
- **Paths are bytes.** Paths cross every boundary as bytes with a lossy form only for display. Non-UTF-8 filenames must round-trip through any action performed on them.
- **Errors carry their origin.** Every error surfaced to the user retains the command line, exit status and verbatim stderr. There is no generic error message.

## Verification gates

| Gate | Milestone | Passes when |
|---|---|---|
| G0 | F-001 Foundation | The app opens the reference superproject and renders real repository state; multiple repositories stay open as independent groups with their own tab strips; the process layer enforces its allowlist, version floor and pool limits; token lint, CI, and the fixture generator all run from a clean checkout. |
| G1 | F-002 Working copy | A real commit can be produced end to end, including line-level staging, hooks, signing and guarded discard, without touching a terminal. |
| G2 | F-003 History and diff | The 100 000-commit fixture meets its render budget with the graph strategy chosen by the R1 spike; diff, blame and file history are complete. |
| G3 | F-004 Branches and sync | The full single-repository daily workflow — branch, merge, rebase, conflict, stash, fetch/pull/push, recovery — needs no CLI fallback. |
| G4 | F-005 Submodule workspace | The reference superproject renders the full matrix in ≤ 1.5 s offline, refreshes 25 remotes in ≤ 10 s, and every bulk operation previews, isolates failure and protects dirty submodules. |
| G5 | F-006 Theming and UX | Token lint passes with zero literals; light and dark meet AA; a user theme switches live; desktop palette sync applies and updates live with a generic label; every action is keyboard-reachable; the app is usable at 1280×720 and 200% text scale. |
| G6 | F-007 Release | AppImage and Arch package launch on Wayland and X11 within size and memory budgets; the no-telemetry and read-only-refresh audits pass. |

## Risks

| ID | Risk | Mitigation |
|---|---|---|
| R1 | WebKitGTK cannot render the commit graph within budget. | Mandatory spike before F-003 comparing DOM, canvas and WebGL against the 100 000-commit fixture. Documented fallback: canvas, then a reduced lane budget with explicit overflow. The graph is the only surface at risk; nothing else in the product depends on this. |
| R2 | Process-spawn overhead dominates once per-submodule calls multiply. | Batch at the superproject level (two calls, not 2N); parallelize the rest; assert the 1.5 s budget in CI so regressions surface immediately. |
| R3 | Output parsing breaks across `git` versions. | Pin a version floor of 2.43, use `--porcelain=v2` and `-z` exclusively, and run the parser suite against multiple `git` versions in CI. |
| R4 | Scope creep toward a full Sourcetree clone stalls the differentiator. | The differentiator is F-005 and its gate is G4. Forge integration, LFS and non-Linux platforms are explicit non-goals; adding any of them requires amending this change. |
| R5 | Bulk operations damage working copies at 25× scale. | Every bulk operation previews per submodule, skips dirty submodules by default, serializes writes per repository, and reports per-row outcomes. No bulk operation may force. |
| R6 | A single-developer project outlives its motivation before the payoff. | Delivery order is chosen so each milestone leaves a usable app, and G4 — the reason the project exists — is reachable without first completing the polish milestones. |
