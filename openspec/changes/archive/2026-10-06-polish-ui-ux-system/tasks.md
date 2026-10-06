# Tasks: polish-ui-ux-system

## 1. Native directory picker

- [x] 1.1 Implement `pick_folder` command in `src-tauri` using `rfd::AsyncFileDialog` - verify: `cargo check -p gittree`
- [x] 1.2 Expose `pickFolder` in frontend API and add "Browse..." buttons in sidebar and empty repository views - verify: `pnpm exec tsc -b` and Vitest pass

## 2. Vector SVG icon system

- [x] 2.1 Implement `<Icon />` component with vector glyphs for folder, settings, search, close, warning, check, dirty, branch, commit, and refresh - verify: `pnpm test` and `pnpm lint:tokens`
- [x] 2.2 Replace all Unicode emoji characters and raw unicode close glyphs across views and modals with `<Icon />` - verify: emoji search returns zero results in frontend source

## 3. Settings and Preferences modal

- [x] 3.1 Implement `SettingsModal.tsx` covering themes, Follow Desktop toggle, network concurrency, and bookmarks - verify: Vitest settings tests pass
- [x] 3.2 Connect Settings modal to header action bar, command palette, and `Ctrl+,` keybinding - verify: triggering `Ctrl+,` opens modal and traps focus

## 4. Verification and Visual QA

- [x] 4.1 Run token linter and verify zero raw color or pixel spacing values - verify: `pnpm lint:tokens` passes
- [x] 4.2 Run full test suite and capture Playwright screenshot of the refreshed UI - verify: `cargo test --workspace` and `pnpm test` pass
