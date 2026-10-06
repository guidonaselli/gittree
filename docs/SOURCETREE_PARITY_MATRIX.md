# Sourcetree Parity Matrix

This document tracks functional parity between GitTree and Sourcetree across core repository operations and specialized workflows.

- `match`: GitTree provides the capability with equivalent or refined ergonomics.
- `exceed`: GitTree offers significant functional advantages beyond Sourcetree.
- `omit`: Deliberately excluded by design (e.g., proprietary hosting lock-in, legacy systems).

| Capability | Status | Implementation Details |
|---|---|---|
| **Repository Bookmarks & Grouping** | `match` | Saved in plain human-editable JSON outside repositories (`~/.config/gittree/bookmarks.json`). |
| **Working Copy Status** | `match` | Grouped view with real-time rename/copy detection, similarity scores, and type change flags. |
| **Hunk-Level Staging** | `match` | Surgical patch construction with preview and reverse unstaging. |
| **Line-Level Staging** | `match` | Partial selection staging; rejects gracefully on conflict rather than corrupting index state. |
| **Commit, Amend, Sign-off & Signing** | `match` | Full GPG/SSH signing verification; signing failures are surfaced rather than downgraded to unsigned. |
| **Hook Output** | `exceed` | Full verbatim stdout and stderr surfaced on hook execution; index remains intact on failure. |
| **Commit Graph** | `match` | Virtualized commit history graph with stable lanes across scrolling and explicit branch collapsing. |
| **Diff Views** | `match` | Side-by-side and unified inline diffs with intra-line word-level highlights. |
| **Word-Level Intra-Line Diff** | `match` | Granular diff highlighting on modified lines. |
| **Interactive Blame** | `exceed` | Attribution blocks per commit; "Blame before this" walks backward through historical parent commits. |
| **File History with Renames** | `match` | Follows file evolution across renames and moves. |
| **History & Content Search** | `match` | Fast regex and content search across commit history with explicit truncation limits. |
| **Branches, Tags & Stashes** | `match` | Complete branch switching, tracking branch configuration, tag management, and stash inspection. |
| **Interactive Rebase** | `match` | Structured rebase plan confirmed before execution with abort/continue controls. |
| **Merge, Cherry-Pick & Revert** | `match` | Guarded operations with strict dirty-tree checks before initiating. |
| **Conflict Resolution** | `match` | Detailed conflict markers with automated delegation to the user's configured system mergetool. |
| **Submodule Gitlink Conflicts** | `exceed` | Shows both conflicting candidate commits with subjects and authors instead of a raw binary conflict. |
| **Fetch, Pull & Push** | `match` | Multi-remote fetch, fast-forward/rebase pull, and explicit tracking branch pushes. |
| **Safe Force Push** | `exceed` | Uses `--force-with-lease` by default; bare force requires explicit separate confirmation. |
| **Reflog Recovery** | `exceed` | First-class recovery view allowing instant navigation and branch recreation from unreferenced commits. |
| **Large Repository Performance** | `exceed` | Virtualized list rendering across working trees and history; verified against 100k-commit fixtures. |
| **Submodule Matrix** | `exceed` | Consolidated real-time matrix covering branch, gitlink divergence, remote tracking, and dirtiness. |
| **Submodule Network Refresh** | `exceed` | Concurrent multi-remote refresh across 25+ submodules in seconds vs serial execution. |
| **Bulk Submodule Operations** | `exceed` | Multi-selection checkout, fetch, pull, and gitlink reset with previews and per-submodule status. |
| **Gitlink Bumping** | `exceed` | Single-click atomic bump of updated submodule SHAs into parent repository commits. |
| **Theme System** | `exceed` | Token-based theming; custom user themes loaded from plain TOML files; desktop mode detection. |
| **Keyboard Accessibility** | `exceed` | 100% keyboard-reachable actions via Command Palette (`Ctrl+P`) and rebindable shortcuts. |
| **Accessibility (a11y)** | `exceed` | WCAG AA contrast compliance, reduced motion support, scalable UI at 200%. |
| **Error Transparency** | `exceed` | Verbatim Git commands, exit codes, and stderr always accessible in the Operation Log. |
| **Git LFS** | `omit` | Out of scope for 1.0 release. |
| **Mercurial Support** | `omit` | Out of scope; focused purely on Git workflows. |
| **Hosting Integrations (PRs/Issues)** | `omit` | Deliberately left to dedicated forge CLIs (`gh`, `glab`, `forge-cli`). |
| **Mandatory Accounts & Licensing** | `omit` | GitTree is 100% free and open-source (MIT); no logins, cloud tiers, or paywalls. |
| **Telemetry & Analytics** | `omit` | Zero telemetry, tracking, or network requests not initiated directly by the user. |
