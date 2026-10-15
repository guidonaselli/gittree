## 1. Sidebar Tree Structure & Submodule Status

- [x] 1.1 Extend `RepositorySidebarProps` to accept submodules array (`SubmoduleState[]`), local branches list (`BranchEntry[]`), and checkout/drill-in handlers - verify: `pnpm --dir frontend run test`
- [x] 1.2 Implement collapsible sections in `RepositorySidebar.tsx` for WORKSPACE, BRANCHES, SUBMODULES, and TAGS/STASHES with persistence and badge counters - verify: `pnpm --dir frontend run test`
- [x] 1.3 Add inline submodule quick view rows displaying name, branch label / detached SHA, gitlink divergence icon, dirty dot, and click-to-open tab handler - verify: `pnpm --dir frontend run test`
- [x] 1.4 Add inline branch rows displaying local branches with current HEAD highlight and double-click checkout - verify: `pnpm --dir frontend run test`

## 2. Token-Based Styling & Responsive Polish

- [x] 2.1 Author styles in `frontend/src/features/repository/repository.css` for sidebar tree headers, sub-items, badges, and active branch indicators using 100% design tokens - verify: `pnpm --dir frontend lint:tokens`
- [x] 2.2 Optimize top titlebar and `SyncToolbar` flex behavior and media queries so controls do not overlap or wrap destructively at >= 940px - verify: `pnpm --dir frontend lint:tokens`
- [x] 2.3 Add unit tests for sidebar tree navigation and selection interactions - verify: `pnpm --dir frontend run test`
- [ ] 2.4 [manual] Visual confirmation on Hyprland desktop at 940px width and dark/light themes
