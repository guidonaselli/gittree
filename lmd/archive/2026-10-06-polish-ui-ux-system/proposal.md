# Proposal: polish-ui-ux-system

## Why
While the repository core and submodule engine meet functional parity, everyday ergonomics are hindered by typing repository paths manually, the absence of a visual settings surface, and the use of raw Unicode emojis instead of crisp, consistent vector SVG icons.

## What Changes
- Add native directory picker command (`pick_folder`) backed by Wayland/X11 desktop portals to allow browsing local folders.
- Add "Browse..." button in the sidebar next to the open repository input and in empty workspace states.
- Introduce an internal lightweight SVG icon system to replace all raw emojis (close ✕, search 🔍, warnings ⚠️, checkmarks ✓, dirty indicators ●) across all views and modals.
- Implement a dedicated Settings / Preferences dialog (`Ctrl+,`) with sections for Theme (with live previews), Git defaults (concurrency, timeouts), and Bookmark management.
- Polish header action bar, status indicators, and modal layouts for an elevated developer tool feel.

## Capabilities

### Modified Capabilities
- `repository-workspace`: Native directory browsing added to open repository workflow.
- `ui-shell-and-theming`: SVG icon system replacing emojis; Settings dialog added to application shell.

## Non-goals
- Modifying Git command execution semantics or repository state data models.
- Adding heavy third-party UI dependencies that inflate bundle size or violate strict offline security.

## Impact
- New Tauri command `pick_folder` in `src-tauri`.
- New UI components: `Icon.tsx`, `SettingsModal.tsx`, and updated `App.tsx`.
- Token lint compliance strictly maintained.
