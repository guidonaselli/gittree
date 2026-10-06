# Design: polish-ui-ux-system

## Context
Ergonomics and visual refinement for GitTree: native directory selection, vector SVG icons replacing emojis, and a dedicated Settings modal. Zero telemetry and offline performance guarantees remain inviolable.

## Decisions

### 1. Directory picker via `rfd` with `xdg-portal`
- **Choice**: Add Tauri command `pick_folder` using `rfd::AsyncFileDialog`.
- **Alternative rejected**: Invoking external CLI helpers (`zenity`, `kdialog`), which are absent on Arch/Hyprland setups.
- **Why**: Native XDG portal integration on Wayland/X11 without external runtime binaries; verified with 0 socket connections in `verify-no-telemetry.sh`.

### 2. Custom lightweight vector SVG icon system
- **Choice**: Implement a standalone SolidJS `<Icon name="..." size="..." />` component supporting standard developer-tool glyphs (`folder`, `settings`, `search`, `close`, `warning`, `check`, `alert`, `branch`, `git-commit`, `refresh`, `chevron`).
- **Alternative rejected**: Installing large third-party icon packages with hundreds of unused icons.
- **Why**: Zero bundle bloat, zero dependencies, uses `currentColor` to seamlessly inherit theme tokens.

### 3. Integrated Preferences Modal (`Ctrl+,`)
- **Choice**: Accessible via top header gear button, command palette, and `Ctrl+,` shortcut. Binds directly to the validated XDG settings backend (`getSettings` / `saveSettings`).
- **Alternative rejected**: Scattered configuration options across different menus.
- **Why**: Centralizes theme selection (with color swatches), follow-desktop toggle, network concurrency, and repository bookmarks.

## Risks
- **Token lint violation**: New CSS rules might introduce raw pixel or hex color values.
  - **Mitigation**: All styles use CSS variables (`--color-*`, `--space-*`, `--radius-*`) and are verified by `pnpm lint:tokens`.
