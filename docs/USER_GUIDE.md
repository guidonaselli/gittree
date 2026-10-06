# GitTree User Guide

GitTree is a high-performance desktop Git client for Linux designed around a submodule workspace and a focused repository daily workflow.

---

## 1. Installation and Launch

GitTree runs natively on both Wayland and X11 without requiring manual configuration.

### Arch Linux

Install the pre-built package:
```bash
sudo pacman -U gittree-1.0.0-1-x86_64.pkg.tar.zst
```

Or build from the source `PKGBUILD`:
```bash
cd packaging/arch
makepkg -si
```

The package installs:
- Binary: `/usr/bin/gittree`
- Desktop entry: `/usr/share/applications/gittree.desktop`
- High-resolution icons in `/usr/share/icons/hicolor/`

### AppImage

Make the AppImage executable and launch:
```bash
chmod +x GitTree_1.0.0_amd64.AppImage
./GitTree_1.0.0_amd64.AppImage
```

### CLI Repository Opening

Open any Git repository or superproject directly from your terminal:
```bash
gittree /path/to/my-repo
```

If launched without arguments, GitTree restores your previously opened repository groups or displays the welcome bookmark view.

---

## 2. Submodule Workspace

The Submodule Workspace provides a consolidated real-time state matrix across all submodules in a superproject.

### State Matrix Columns

- **Submodule**: Relative path and name of the submodule.
- **Branch**: Currently checked out branch or detached commit SHA.
- **Gitlink Divergence**: Relationship between the submodule HEAD and the commit recorded in the superproject index (e.g. `in-sync`, `ahead +2`, `behind -5`).
- **Remote Divergence**: Ahead/behind commit counts relative to upstream tracking ref (`@{u}` or fallback remote branch).
- **Working Tree State**: Clean or dirty (with modified, staged, or untracked counts).
- **Last Fetch**: Elapsed time since the last network fetch.

### Parallel Network Refresh

Fetching status across dozens of remotes is executed concurrently in parallel worker threads (configurable, default 8). A full 25-submodule superproject refresh settles within seconds rather than minutes.

### Bulk Operations

Select one, multiple, or all submodules to execute coordinated actions:
- **Bulk Checkout**: Switch selected submodules to a branch, with dirty-tree safety checks.
- **Bulk Fetch / Pull**: Fast concurrent network updates with per-row outcomes and previews.
- **Bulk Gitlink Reset**: Align submodule worktrees back to the exact commit recorded by the parent repository.

### One-Click Gitlink Bump

When a submodule has advanced, bump its recorded commit in the parent repository with a single action, creating an atomic commit in the superproject with an unreachable-commit warning guard.

---

## 3. Themes and Appearance

GitTree uses a token-only design architecture. Color values and UI metrics flow entirely through design tokens.

### Built-in Themes

- **Dark**: Default high-contrast dark theme meeting WCAG AA standards.
- **Light**: Crisp, accessible light theme.
- **System**: Automatically matches your desktop's light/dark mode preference via FreeDesktop portals.

### Authoring Custom User Themes

User themes are plain TOML files stored in your XDG configuration directory:
```
~/.config/gittree/themes/<theme-name>.toml
```

Example theme (`~/.config/gittree/themes/nord.toml`):
```toml
name = "Nord Dark"
mode = "dark"

[tokens]
--color-bg = "#2e3440"
--color-surface = "#3b4252"
--color-surface-hover = "#434c5e"
--color-text = "#eceff4"
--color-text-muted = "#d8dee9"
--color-border = "#4c566a"
--color-accent = "#88c0d0"
--color-accent-hover = "#81a1c1"
--color-diff-add = "#a3be8c"
--color-diff-del = "#bf616a"
```

Custom themes appear immediately in GitTree Settings under the Theme dropdown. Any syntax or token errors are reported with file name and line numbers without crashing the application.

---

## 4. Keybindings and Command Palette

GitTree is fully operable from the keyboard.

### Command Palette

Press `Ctrl+P` or `Ctrl+K` to open the Command Palette.
- Filter any command or repository view by typing.
- View assigned keyboard shortcuts for each action.
- Actions that are currently unavailable are displayed with an explanatory reason.

### Customizing Keybindings

Open Settings (`Ctrl+,`) and navigate to the **Keybindings** tab to customize shortcuts:
- Click any action to record a new key combination.
- Conflict detection warns immediately if a shortcut collides with an existing action.
- "Reset to Defaults" restores standard factory keybindings.

---

## 5. Settings and Persistence

GitTree configuration is stored in hand-editable plain JSON under XDG standards:
```
~/.config/gittree/settings.json
~/.config/gittree/bookmarks.json
```

### Settings Schema (`settings.json`)

```json
{
  "concurrency": 8,
  "theme": null
}
```

- `concurrency`: Maximum number of parallel Git processes for background tasks and bulk operations (integer > 0).
- `theme`: Theme identifier string, or `null` to follow the desktop environment.

### Fault Tolerance

Settings are validated per-key on startup. If a single key is malformed or invalid (e.g. negative concurrency), GitTree falls back to the default value for that key while preserving all other valid settings, recording a clear warning in the log.

---

## 6. Privacy and Offline Guarantees

- **Zero Telemetry**: No outbound network requests at startup or during idle. No analytics, tracking, crash telemetry, or third-party pings.
- **Full Offline Usability**: All repository views, staging, diffs, blame, commits, and logs function completely without an internet connection. Remote operations fail cleanly with descriptive errors when the network is unavailable.
- **Safe by Default**: Background watchers and refreshes are strictly read-only. No write operation is executed without explicit user confirmation.
