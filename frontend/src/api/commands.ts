import { invoke } from "@tauri-apps/api/core";
import type {
  AbortOutcome,
  ActiveOperationDetail,
  Bookmark,
  BranchComparison,
  BranchEntry,
  CheckoutOutcome,
  CherryPickOptions,
  CherryPickOutcome,
  CommitSummary,
  ConflictItem,
  ConflictMarkerInfo,
  ConflictResolution,
  CreateStashOptions,
  CreateTagOptions,
  DeleteBranchOutcome,
  DesktopPalette,
  DirtyTreeDetails,
  FetchOptions,
  FileDiff,
  IgnoreExplanation,
  IgnoreTarget,
  LogEntry,
  MergeOptions,
  MergeOutcome,
  MergetoolConfig,
  MergetoolOutcome,
  MultiRemoteFetchResult,
  OpenRepositoryResult,
  OperationStepOutcome,
  PullOptions,
  PullOutcome,
  PushOptions,
  PushOutcome,
  PushTagOptions,
  RebaseOutcome,
  RebasePlanItem,
  RemoteInfo,
  ReflogEntry,
  ResetOutcome,
  ResetReflogOptions,
  RepositoryState,
  Resolved,
  RevertOptions,
  RevertOutcome,
  Settings,
  SettingsLoadResult,
  StageOutcome,
  StashApplyOutcome,
  StashDetail,
  StashEntry,
  SubmoduleState,
  TagEntry,
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

export function cancelWorkingCopyStatus(): Promise<void> {
  return invoke<void>("cancel_working_copy_status");
}

export function checkIgnorePath(root: string, path: string): Promise<IgnoreExplanation | null> {
  return invoke<IgnoreExplanation | null>("check_ignore_path", { root, path });
}

export function addIgnoreRule(root: string, target: IgnoreTarget, pattern: string): Promise<void> {
  return invoke<void>("add_ignore_rule_command", { root, target, pattern });
}

export function stageWorkingCopyPaths(
  root: string,
  paths: string[],
  overrideMarkers = false
): Promise<StageOutcome> {
  return invoke<StageOutcome>("stage_working_copy_paths", { root, paths, overrideMarkers });
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

export type { CommitSummary } from "./types";

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

export type ContentSearchMode = "Pickaxe" | "Regex";

export interface HistorySearchOptions {
  scope?: HistoryScope;
  message?: string;
  author?: string;
  path?: string;
  since?: string;
  until?: string;
  content_query?: string;
  content_mode?: ContentSearchMode;
  skip?: number;
  limit?: number;
}

export interface HistorySearchResult {
  commits: CommitSummary[];
  truncated: boolean;
}

export function searchHistory(root: string, options: HistorySearchOptions): Promise<HistorySearchResult> {
  return invoke<HistorySearchResult>("search_history", { root, options });
}

export function cancelHistorySearch(): Promise<void> {
  return invoke<void>("cancel_history_search");
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

export function getBranches(root: string): Promise<BranchEntry[]> {
  return invoke<BranchEntry[]>("get_branches", { root });
}

export function createBranch(
  root: string,
  name: string,
  startPoint: string | null = null,
  checkout = false
): Promise<void> {
  return invoke<void>("create_branch_command", {
    root,
    name,
    startPoint,
    checkout,
  });
}

export function createTrackingBranch(
  root: string,
  name: string,
  remoteBranch: string,
  checkout = false
): Promise<void> {
  return invoke<void>("create_tracking_branch_command", {
    root,
    name,
    remoteBranch,
    checkout,
  });
}

export function renameBranch(
  root: string,
  oldName: string,
  newName: string
): Promise<void> {
  return invoke<void>("rename_branch_command", {
    root,
    oldName,
    newName,
  });
}

export function deleteBranch(
  root: string,
  name: string,
  force = false
): Promise<DeleteBranchOutcome> {
  return invoke<DeleteBranchOutcome>("delete_branch_command", {
    root,
    name,
    force,
  });
}

export function checkoutBranch(
  root: string,
  name: string
): Promise<CheckoutOutcome> {
  return invoke<CheckoutOutcome>("checkout_branch_command", {
    root,
    name,
  });
}

export function stashAndCheckout(
  root: string,
  target: string,
  message: string | null = null
): Promise<void> {
  return invoke<void>("stash_and_checkout_command", {
    root,
    target,
    message,
  });
}

export function compareBranches(
  root: string,
  base: string,
  target: string
): Promise<BranchComparison> {
  return invoke<BranchComparison>("compare_branches_command", {
    root,
    base,
    target,
  });
}

// Tags
export function getTags(root: string): Promise<TagEntry[]> {
  return invoke<TagEntry[]>("get_tags", { root });
}

export function createTag(root: string, opts: CreateTagOptions): Promise<TagEntry> {
  return invoke<TagEntry>("create_tag_command", { root, opts });
}

export function deleteTag(root: string, name: string): Promise<void> {
  return invoke<void>("delete_tag_command", { root, name });
}

export function pushTag(root: string, opts: PushTagOptions): Promise<void> {
  return invoke<void>("push_tag_command", { root, opts });
}

export function deleteRemoteTag(root: string, remote: string, name: string): Promise<void> {
  return invoke<void>("delete_remote_tag_command", { root, remote, name });
}

export function getRemoteTags(root: string, remote: string): Promise<string[]> {
  return invoke<string[]>("get_remote_tags", { root, remote });
}

// Stashes
export function getStashes(root: string): Promise<StashEntry[]> {
  return invoke<StashEntry[]>("get_stashes", { root });
}

export function inspectStash(root: string, selector: string): Promise<StashDetail> {
  return invoke<StashDetail>("inspect_stash_command", { root, selector });
}

export function createStash(root: string, opts: CreateStashOptions): Promise<string> {
  return invoke<string>("create_stash_command", { root, opts });
}

export function applyStash(
  root: string,
  selector: string,
  reinstateIndex = false
): Promise<StashApplyOutcome> {
  return invoke<StashApplyOutcome>("apply_stash_command", {
    root,
    selector,
    reinstateIndex,
  });
}

export function popStash(
  root: string,
  selector: string,
  reinstateIndex = false
): Promise<StashApplyOutcome> {
  return invoke<StashApplyOutcome>("pop_stash_command", {
    root,
    selector,
    reinstateIndex,
  });
}

export function dropStash(root: string, selector: string): Promise<void> {
  return invoke<void>("drop_stash_command", { root, selector });
}

export function clearStashes(root: string): Promise<void> {
  return invoke<void>("clear_stashes_command", { root });
}

// Integration Operations
export function getActiveOperation(root: string): Promise<ActiveOperationDetail | null> {
  return invoke<ActiveOperationDetail | null>("get_active_operation", { root });
}

export function getConflictingFiles(root: string): Promise<string[]> {
  return invoke<string[]>("get_conflicting_files", { root });
}

export function checkDirtyWorkingCopy(root: string): Promise<DirtyTreeDetails | null> {
  return invoke<DirtyTreeDetails | null>("check_dirty_working_copy", { root });
}

export function startMerge(
  root: string,
  targetRef: string,
  options: MergeOptions
): Promise<MergeOutcome> {
  return invoke<MergeOutcome>("start_merge_command", { root, targetRef, options });
}

export function startCherryPick(
  root: string,
  commitRef: string,
  options: CherryPickOptions
): Promise<CherryPickOutcome> {
  return invoke<CherryPickOutcome>("start_cherry_pick_command", { root, commitRef, options });
}

export function startRevert(
  root: string,
  commitRef: string,
  options: RevertOptions
): Promise<RevertOutcome> {
  return invoke<RevertOutcome>("start_revert_command", { root, commitRef, options });
}

export function getRebasePlan(root: string, baseRef: string): Promise<RebasePlanItem[]> {
  return invoke<RebasePlanItem[]>("get_rebase_plan", { root, baseRef });
}

export function startInteractiveRebase(
  root: string,
  baseRef: string,
  plan: RebasePlanItem[]
): Promise<RebaseOutcome> {
  return invoke<RebaseOutcome>("start_interactive_rebase_command", { root, baseRef, plan });
}

export function continueOperation(root: string): Promise<OperationStepOutcome> {
  return invoke<OperationStepOutcome>("continue_operation_command", { root });
}

export function skipOperation(root: string): Promise<OperationStepOutcome> {
  return invoke<OperationStepOutcome>("skip_operation_command", { root });
}

export function abortOperation(root: string): Promise<AbortOutcome> {
  return invoke<AbortOutcome>("abort_operation_command", { root });
}

export function getConflicts(root: string): Promise<ConflictItem[]> {
  return invoke<ConflictItem[]>("get_conflicts", { root });
}

export function checkConflictMarkers(root: string, path: string): Promise<ConflictMarkerInfo | null> {
  return invoke<ConflictMarkerInfo | null>("check_conflict_markers_command", { root, path });
}

export function resolveConflict(
  root: string,
  path: string,
  resolution: ConflictResolution
): Promise<void> {
  return invoke<void>("resolve_conflict_command", { root, path, resolution });
}

export function launchMergetool(
  root: string,
  path: string,
  tool?: string
): Promise<MergetoolOutcome> {
  return invoke<MergetoolOutcome>("launch_mergetool_command", { root, path, tool });
}

export function getMergetoolConfig(root: string): Promise<MergetoolConfig> {
  return invoke<MergetoolConfig>("get_mergetool_config", { root });
}

export function getRemotes(root: string): Promise<RemoteInfo[]> {
  return invoke<RemoteInfo[]>("get_remotes", { root });
}

export function fetchRemotes(root: string, options: FetchOptions): Promise<MultiRemoteFetchResult> {
  return invoke<MultiRemoteFetchResult>("fetch_remotes", { root, options });
}

export function pullRepository(root: string, options: PullOptions): Promise<PullOutcome> {
  return invoke<PullOutcome>("pull_repository", { root, options });
}

export function pushRepository(root: string, options: PushOptions): Promise<PushOutcome> {
  return invoke<PushOutcome>("push_repository", { root, options });
}

export function cancelSyncNetworkOperation(): Promise<void> {
  return invoke<void>("cancel_sync_network_operation");
}

export function submitAskpassResponse(id: string, response: string): Promise<boolean> {
  return invoke<boolean>("submit_askpass_response", { id, response });
}

export function cancelAskpassResponse(id: string): Promise<boolean> {
  return invoke<boolean>("cancel_askpass_response", { id });
}

export function getReflog(
  root: string,
  refTarget?: string | null,
  limit?: number | null
): Promise<ReflogEntry[]> {
  return invoke<ReflogEntry[]>("get_reflog", {
    root,
    ref_target: refTarget ?? null,
    limit: limit ?? null,
  });
}

export function resetToReflog(
  root: string,
  options: ResetReflogOptions
): Promise<ResetOutcome> {
  return invoke<ResetOutcome>("reset_to_reflog", { root, options });
}



