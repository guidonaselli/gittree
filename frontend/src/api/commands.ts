import { invoke } from "@tauri-apps/api/core";
import type {
  Bookmark,
  DesktopPalette,
  LogEntry,
  OpenRepositoryResult,
  RepositoryState,
  Resolved,
  Settings,
  SettingsLoadResult,
  SubmoduleState,
  WorkingCopyStatus,
} from "./types";

// Thin wrapper over the Tauri commands.

export function resolveRepositoryRoot(path: string): Promise<OpenRepositoryResult> {
  return invoke<OpenRepositoryResult>("resolve_repository_root", { path });
}

export function initRepository(path: string): Promise<string> {
  return invoke<string>("init_repository", { path });
}

export function getRepositoryState(root: string): Promise<RepositoryState> {
  return invoke<RepositoryState>("get_repository_state", { root });
}

export function getSubmoduleMatrix(root: string): Promise<SubmoduleState[]> {
  return invoke<SubmoduleState[]>("get_submodule_matrix", { root });
}

export function getWorkingCopyStatus(root: string): Promise<Resolved<WorkingCopyStatus>> {
  return invoke<Resolved<WorkingCopyStatus>>("get_working_copy_status", { root });
}

export function getSettings(): Promise<SettingsLoadResult> {
  return invoke<SettingsLoadResult>("get_settings");
}

export function saveSettings(settings: Settings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

export function getBookmarks(): Promise<Bookmark[]> {
  return invoke<Bookmark[]>("get_bookmarks");
}

export function addBookmark(root: string, group: string | null): Promise<Bookmark[]> {
  return invoke<Bookmark[]>("add_bookmark", { root, group });
}

export function removeBookmark(root: string): Promise<Bookmark[]> {
  return invoke<Bookmark[]>("remove_bookmark", { root });
}

export function startWatching(root: string): Promise<void> {
  return invoke<void>("start_watching", { root });
}

export function stopWatching(root: string): Promise<void> {
  return invoke<void>("stop_watching", { root });
}

export function getOperationLog(): Promise<LogEntry[]> {
  return invoke<LogEntry[]>("get_operation_log");
}

export function getDesktopPalette(): Promise<DesktopPalette | null> {
  return invoke<DesktopPalette | null>("get_desktop_palette");
}
