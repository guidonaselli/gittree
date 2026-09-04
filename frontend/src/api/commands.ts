import { invoke } from "@tauri-apps/api/core";
import type {
  Bookmark,
  DesktopPalette,
  FileDiff,
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

export function stageWorkingCopyPaths(root: string, paths: string[]): Promise<void> {
  return invoke<void>("stage_working_copy_paths", { root, paths });
}

export function unstageWorkingCopyPaths(root: string, paths: string[]): Promise<void> {
  return invoke<void>("unstage_working_copy_paths", { root, paths });
}

export function getFileDiff(root: string, path: string, staged: boolean): Promise<FileDiff | null> {
  return invoke<FileDiff | null>("get_file_diff", { root, path, staged });
}

export function stageFileHunks(root: string, path: string, hunkIndices: number[]): Promise<void> {
  return invoke<void>("stage_file_hunks", { root, path, hunkIndices });
}

export function unstageFileHunks(root: string, path: string, hunkIndices: number[]): Promise<void> {
  return invoke<void>("unstage_file_hunks", { root, path, hunkIndices });
}

export function stageFileLines(
  root: string,
  path: string,
  hunkIndex: number,
  lineIndices: number[],
): Promise<void> {
  return invoke<void>("stage_file_lines", { root, path, hunkIndex, lineIndices });
}

export function unstageFileLines(
  root: string,
  path: string,
  hunkIndex: number,
  lineIndices: number[],
): Promise<void> {
  return invoke<void>("unstage_file_lines", { root, path, hunkIndex, lineIndices });
}

export function discardWorkingCopyPaths(root: string, paths: string[]): Promise<void> {
  return invoke<void>("discard_working_copy_paths", { root, paths });
}

export function deleteUntrackedWorkingCopyPaths(root: string, paths: string[]): Promise<void> {
  return invoke<void>("delete_untracked_working_copy_paths", { root, paths });
}

export function stashWorkingCopyPaths(root: string, paths: string[], message?: string): Promise<void> {
  return invoke<void>("stash_working_copy_paths", { root, paths, message: message ?? null });
}

export interface CommitOptions {
  author?: string;
  signOff: boolean;
  sign: boolean;
}

export function commitWorkingCopy(root: string, message: string, options: CommitOptions): Promise<void> {
  return invoke<void>("commit_working_copy", {
    root,
    message,
    author: options.author ?? null,
    signOff: options.signOff,
    sign: options.sign,
  });
}

export function isHeadPublished(root: string): Promise<boolean> {
  return invoke<boolean>("is_head_published", { root });
}

export function amendWorkingCopy(root: string, message: string, options: CommitOptions): Promise<void> {
  return invoke<void>("amend_working_copy", {
    root,
    message,
    author: options.author ?? null,
    signOff: options.signOff,
    sign: options.sign,
  });
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
