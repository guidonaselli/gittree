Project: GitTree, a desktop Git client for Linux that replaces Atlassian Sourcetree
(macOS/Windows only) and improves on it where Sourcetree is weak.
Primary platform: Linux/Wayland (Hyprland, CachyOS reference machine); X11 must also work.
Stack: Tauri 2 shell, Rust backend, web frontend. Git data comes from shelling out to the
system `git` CLI (>= 2.43) through a bounded worker pool — never libgit2 or gitoxide,
because `gix` has documented gaps exactly in submodule status, which is this product's
differentiator, and the `git` CLI is the free correctness reference.
Reference workload: a 25-submodule, 3.1 GB superproject where submodules sit on five
different branch families at once, every submodule diverges from the recorded gitlink,
upstream tracking (@{u}) is unset on most, and one is detached.
Differentiator: a submodule workspace no existing Linux client provides — every submodule's
branch, gitlink divergence, remote divergence and dirtiness in one always-current matrix.
Delivery order: core Git client first (working copy, history, branches/sync), submodule
workspace next, theming polish after. Sourcetree feature parity is the floor, not the ceiling.
Planning model: OpenSpec is canonical for requirements and design; LMD under 05_PLANNING
mirrors execution state and links back to OpenSpec task numbers.
