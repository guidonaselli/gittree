<div align="center">
  <img src="frontend/public/logo.png" alt="GitTree Logo" width="120" />
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
