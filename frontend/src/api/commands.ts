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

export function getBlobBase64(root: string, sha: string): Promise<string> {
  return invoke<string>("get_blob_base64", { root, sha });
}

export function getWorkingTreeFileBase64(root: string, path: string): Promise<string> {
  return invoke<string>("get_working_tree_file_base64", { root, path });
}

export interface BlameLine {
  sha: string;
  author_name: string;
  author_email: string;
  author_time: number;
  summary: string;
  orig_line_no: number;
  final_line_no: number;
  content: string;
  is_boundary: boolean;
}

export function getBlame(
  root: string,
  path: string,
  ignoreWhitespace: boolean,
  rev?: string,
): Promise<BlameLine[]> {
  return invoke<BlameLine[]>("get_blame", { root, path, ignoreWhitespace, rev: rev ?? null });
}

export function getFileDiffWithOptions(
  root: string,
  path: string,
  staged: boolean,
  contextLines: number | undefined,
  ignoreWhitespace: boolean,
): Promise<FileDiff | null> {
  return invoke<FileDiff | null>("get_file_diff_with_options", {
    root,
    path,
    staged,
    contextLines: contextLines ?? null,
    ignoreWhitespace,
  });
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

export function commitWorkingCopy(root: string, message: string, options: CommitOptions): Promise<string> {
  return invoke<string>("commit_working_copy", {
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

export function amendWorkingCopy(root: string, message: string, options: CommitOptions): Promise<string> {
  return invoke<string>("amend_working_copy", {
    root,
    message,
    author: options.author ?? null,
    signOff: options.signOff,
    sign: options.sign,
  });
}

export interface CommitMessageTemplate {
  content: string;
  comment_char: string;
}

export function getCommitMessageTemplate(root: string): Promise<CommitMessageTemplate | null> {
  return invoke<CommitMessageTemplate | null>("get_commit_message_template", { root });
}

export type HistoryScope =
  | { kind: "CurrentBranch" }
  | { kind: "AllBranches" }
  | { kind: "AllRefs" }
  | { kind: "Path"; path: string };

export interface CommitSummary {
  sha: string;
  parents: string[];
  author_name: string;
  author_email: string;
  author_date: string;
  subject: string;
  rename_from?: string | null;
  path_at_commit?: string | null;
}

export interface HistoricalFile {
  content: string;
  is_binary: boolean;
  size: number;
}

export function getRevisionDiff(
  root: string,
  oldRev: string,
  oldPath: string | null | undefined,
  newRev: string,
  newPath: string | null | undefined,
  contextLines?: number,
  ignoreWhitespace?: boolean,
): Promise<FileDiff[]> {
  return invoke<FileDiff[]>("get_revision_diff", {
    root,
    oldRev,
    oldPath: oldPath ?? null,
    newRev,
    newPath: newPath ?? null,
    contextLines: contextLines ?? null,
    ignoreWhitespace: ignoreWhitespace ?? false,
  });
}

export function getFileAtRevision(
  root: string,
  rev: string,
  path: string,
): Promise<HistoricalFile> {
  return invoke<HistoricalFile>("get_file_at_revision", { root, rev, path });
}

export function getHistoryPage(
  root: string,
  scope: HistoryScope,
  skip: number,
  limit: number,
): Promise<CommitSummary[]> {
  return invoke<CommitSummary[]>("get_history_page", { root, scope, skip, limit });
}

export function getHistoryCount(root: string, scope: HistoryScope): Promise<number> {
  return invoke<number>("get_history_count", { root, scope });
}

export interface GraphRow {
  sha: string;
  parents: string[];
  lane: number;
  parent_lanes: number[];
  is_merge: boolean;
  overflow: boolean;
}

export interface GraphResult {
  rows: GraphRow[];
  max_lane: number;
}

export function getHistoryGraph(root: string, scope: HistoryScope): Promise<GraphResult> {
  return invoke<GraphResult>("get_history_graph", { root, scope });
}

export type SignatureState =
  | "Unsigned"
  | { Valid: { signer: string } }
  | { Invalid: { reason: string } }
  | { Unverifiable: { reason: string } };

export interface FileStat {
  path: string;
  additions: number | null;
  deletions: number | null;
}

export interface CommitDetail {
  sha: string;
  parents: string[];
  author_name: string;
  author_email: string;
  author_date: string;
  committer_name: string;
  committer_email: string;
  committer_date: string;
  decorations: string[];
  subject: string;
  body: string;
  signature: SignatureState;
  diff_parent_index: number | null;
  files: FileStat[];
}

export function getCommitDetail(root: string, sha: string, parentIndex?: number): Promise<CommitDetail> {
  return invoke<CommitDetail>("get_commit_detail", { root, sha, parentIndex: parentIndex ?? null });
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
