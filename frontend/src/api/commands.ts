import { invoke } from "@tauri-apps/api/core";
import type { RepositoryState, SubmoduleState } from "./types";

// Thin wrapper over the three Tauri commands exposed today. Every UI surface
// goes through here rather than calling `invoke` directly, so the argument
// shape only needs to match Rust in one place (design D3: "no UI surface
// queries Git directly" extends to "no UI surface talks to Tauri directly").

export function resolveRepositoryRoot(path: string): Promise<string> {
  return invoke<string>("resolve_repository_root", { path });
}

export function getRepositoryState(root: string): Promise<RepositoryState> {
  return invoke<RepositoryState>("get_repository_state", { root });
}

export function getSubmoduleMatrix(root: string): Promise<SubmoduleState[]> {
  return invoke<SubmoduleState[]>("get_submodule_matrix", { root });
}
