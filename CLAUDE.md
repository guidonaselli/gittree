# GitTree

Desktop Git client for Linux (Tauri 2 + Rust + Solid) whose differentiator is a live submodule workspace matrix. Layout, dev, test and build commands: `README.md`.

## Where the truth lives

- Planning: `openspec/changes/build-gittree-client/`. `tasks.md` is the only progress record (task numbers like `1.16`, gates `G0`..`G6`); `proposal.md`, `design.md` and `specs/` hold requirements and decisions.
- Project context and authoring rules: `openspec/config.yaml`.
- Status: `openspec list` plus `tasks.md`. Never copy task status into another file.
- Read `design.md` and the relevant `specs/<area>/spec.md` before changing behavior in that area; do not read the whole change up front.

## Hard rules

- `git` runs only through `crates/git-process` (argv-only, no shell, read-only allowlist). No libgit2 or gitoxide.
- Any requirement touching repository state keeps its destructive-action guard.
- Fully usable offline; no telemetry or paid/cloud dependency. Justify every new dependency (stdlib first).
- Styling is token-only (`frontend/src/theme/tokens.css`); `pnpm --dir frontend lint:tokens` fails on literal colors or raw spacing.
- Every task is proven by the command or check named in `tasks.md`; run it before marking the task done.

## Branding

- Read `branding/README.md` before using or adding any logo, icon or favicon.
- `branding/`, `src-tauri/icons/` and the logo files in `frontend/public/` are generated. Change `branding/source/concept.png` or `scripts/build-brand.py`, then run `scripts/build-brand.sh`.
- Pick the icon by rendered size: full mark from 64 px, small mark at 48 px, tiny mark (no penguin) at 32 px and below.
