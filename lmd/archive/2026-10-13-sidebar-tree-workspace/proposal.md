## Why

The current repository navigation sidebar displays flat view buttons with no at-a-glance visibility into branches or submodules. Users coming from workflows like Sourcetree require instant visual status of active branches and submodules directly in the sidebar without navigating away from the working copy or history view, while keeping the client lightweight and avoiding UI clutter.

## What Changes

- Redesign `RepositorySidebar` to replace the flat navigation list with hierarchical, collapsible tree sections inspired by Sourcetree ergonomics:
  - **WORKSPACE**: Working Copy (with dirty counter badge) and History log.
  - **BRANCHES**: Collapsible section showing local branches inline with active HEAD indicator, one-click checkout or context actions, and an expandable "View all / remotes..." shortcut.
  - **SUBMODULES**: Collapsible section showing each submodule with its current branch/commit, compact gitlink divergence indicator (ahead/behind/synced/dirty), and one-click navigation to open the submodule in a workspace tab.
  - **TAGS & STASHES**: Compact collapsible sections with item counts.
- Add search filter inside the sidebar for quick filtering of branches and submodules when lists are long (> 10 items).
- Optimize titlebar and sync toolbar responsiveness so repository tabs, sync controls, and window actions adapt gracefully without clipping or overlapping at standard Linux desktop window sizes (>= 940px width).

## Capabilities

### Modified Capabilities
- `ui-shell-and-theming`: Add hierarchical tree sidebar navigation with collapsible sections, branch quick-switch, and inline submodule status items adhering to design tokens.
- `repository-workspace`: Enable direct submodule tab opening and branch switching directly from the repository sidebar tree.

## Non-goals

- Re-implementing Sourcetree's heavy multi-pane dialogs or proprietary Atlassian login/cloud services.
- Adding arbitrary file tree explorer or IDE features to the sidebar (GitTree is a Git client, not a text editor).
- Modifying backend CLI execution primitives; all status and branch checkout actions reuse existing `crates/git-process` commands.

## Impact

- **Frontend UI**: Refactor `RepositorySidebar.tsx` and `repository.css`. Updates integrate cleanly with `useWorkspace` and `RepositoryState`.
- **Design Tokens**: 100% adherence to `frontend/src/theme/tokens.css` with zero literal colors or raw px spacing.
- **Performance Budget**: Sidebar re-render takes <= 16ms on a 25-submodule repository; tree expansion state persists across session tabs.
- **Rollback**: Low risk, localized entirely to frontend layout components.
