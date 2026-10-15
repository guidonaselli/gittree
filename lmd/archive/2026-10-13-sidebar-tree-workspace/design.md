## Context

See `proposal.md` for background and motivation. The goal is to transform `RepositorySidebar` from a flat view switcher into a hierarchical, collapsible workspace tree reflecting Sourcetree's best ergonomics (inline branches, submodules with branch and divergence indicators, and direct tab opening) while honoring all design token invariants and offline performance budgets.

## Decisions

### 1. Unified Tree Navigation Data Flow
- **Choice**: Feed existing cached resource signals (`repoState`, `submodules`, `branches`, `workingCopy`) down into `RepositorySidebar` without introducing new backend calls or background polling loops.
- **Alternative rejected**: Fetching submodule and branch lists on demand inside the sidebar component.
- **Why**: `App.tsx` already has reactive caches (`submoduleCache`, `repoCache`) and file-watching invalidation. Reusing them eliminates redundant `git` CLI process spawns and guarantees 0ms extra lag when opening the sidebar sections.

### 2. Collapsible Section Hierarchy
- **Choice**:
  - `WORKSPACE`: Working copy (with staged/unstaged badge) & History.
  - `BRANCHES`: Collapsible list of local branches. Active HEAD has bold accent styling and a check icon. Double-clicking executes safe checkout with existing conflict guards. A "View all branches..." button switches to the full `BranchesView`.
  - `SUBMODULES`: Collapsible list of all submodules. Each item shows:
    - Relative name.
    - Branch badge or commit SHA (e.g. `main` or `detached @ a1b2c3d`).
    - Divergence signal icon (synced check, ahead arrow, behind arrow, or dirty indicator).
    - Clicking drills in directly via `workspace.openSubmoduleTab()`.
    - A "Matrix view" button opens the complete `SubmoduleMatrix` view.
  - `TAGS & STASHES`: Lightweight summary rows with item counters that switch to their respective full views on click.
- **Alternative rejected**: Replicating Sourcetree's exact Windows WPF multi-level tree with full remote branch tracking trees inside the sidebar.
- **Why**: Overcomplicating with deep remote hierarchies in the sidebar introduces visual noise and horizontal scroll issues in narrower windows. Keeping local branches and submodules front and center solves the user's primary need cleanly.

### 3. Quick Filter
- **Choice**: An optional search/filter input at the top of the sidebar section when branches or submodules count > 8 items.
- **Alternative rejected**: Always-visible multi-search or separate search per section.
- **Why**: Keeps UI clean and focused on standard repositories while providing instant keyboard navigation on large projects (e.g. 25+ submodules).

### 4. Top Sync Toolbar & Titlebar Responsiveness
- **Choice**: Set flex-shrink and compact icon/text styling for `SyncToolbar` (`Fetch`, `Pull`, `Push`) with abbreviated labels on narrow viewports (`>= 940px`).
- **Alternative rejected**: Moving sync buttons into a separate menu.
- **Why**: Sourcetree users value 1-click access to Fetch/Pull/Push with immediate ahead/behind counters.

## Risks

- **Risk**: Rendering 25+ submodules with branch status causing layout recalculations or sluggishness.
  - **Mitigation**: SolidJS fine-grained `<For>` reactivity renders each item once; tokens ensure CSS layout stays fixed height (`var(--space-6)` / 24-28px per row).
- **Risk**: Accidentally triggering destructive checkout on double-click.
  - **Mitigation**: Double-click checkout pipes directly into `checkoutBranch` which respects existing dirty-tree refuse guards (`G2` gate).
