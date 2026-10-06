<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="branding/logo-dark.svg" />
    <img src="branding/logo.svg" alt="GitTree Logo" width="180" />
  </picture>
  <h1>GitTree</h1>
  <p>A fast desktop Git client (Tauri 2 + Rust + Solid) built around a submodule workspace: one live matrix of every submodule's branch, its divergence from the commit the superproject records, its divergence from its remote, and its dirtiness.</p>
</div>


## Develop

```
pnpm --dir frontend install
cargo run -p gittree
```

`cargo run` launches the Tauri shell, which in a debug build loads the frontend from the Vite dev server (`beforeDevCommand` in `src-tauri/tauri.conf.json`). Run `pnpm --dir frontend dev` in a separate terminal first if the shell doesn't start it for you.

## Test

```
cargo test --workspace
cargo test --workspace -- --ignored   # performance fixtures (generates real repos)
pnpm --dir frontend test
pnpm --dir frontend lint:tokens
```

## Build

```
pnpm --dir frontend build
cargo build --workspace --release
```

## Layout

- `crates/git-process` — the only place `git` is invoked: argv-only, bounded worker pool, read-only allowlist.
- `crates/repo-state` — repository and submodule state model.
- `src-tauri` — the Tauri application: commands, settings/bookmarks persistence, filesystem watcher.
- `frontend` — Solid + TypeScript UI, token-only styling (`frontend/src/theme/tokens.css`).

## Documentation

- [User Guide](docs/USER_GUIDE.md) — Installation, Submodule Workspace, Custom Themes, Keybindings, and Settings.
- [Sourcetree Parity Matrix](docs/SOURCETREE_PARITY_MATRIX.md) — Feature parity and architectural comparisons.

## Packaging & Release Verification

Automated release gates verify binary size, memory footprint, packaging integrity, and privacy guarantees:

```bash
bash scripts/verify-packaging.sh         # AppImage and Arch package launch verification
bash scripts/verify-release-budgets.sh   # Binary size (<=40MB) and idle memory (<=250MB) gate
bash scripts/verify-no-telemetry.sh      # Zero outbound telemetry / network socket verification
bash scripts/verify-offline-usability.sh # 100% offline workflow execution check
```

## License

MIT License. See [LICENSE](LICENSE) for details.
