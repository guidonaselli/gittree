// Mirrors the serde_json shape of crates/repo-state's public types, plus
// src-tauri's settings/bookmarks types. Kept hand-written and in one place
// so a Rust field rename is a compile error here, not a silent `undefined`
// in the UI.

export type Bookmark = { root: string; group: string | null; order: number };

export type Settings = { concurrency: number; theme: string | null };
export type SettingsLoadResult = { settings: Settings; warnings: string[] };

export type LogEntry = {
  repo_root: string;
  args: string[];
  exit_status: number;
  duration_ms: number;
  timestamp_unix_ms: number;
  write: boolean;
};

export type DesktopPalette = { mode: string; tokens: Record<string, string> };

export type OpenRepositoryResult =
  | { kind: "Repository"; root: string }
  | { kind: "BareRepository"; root: string }
  | { kind: "NotARepository" };

export type Resolved<T> = { state: "Known"; value: T } | { state: "Unknown"; value: { reason: string } };

export function isKnown<T>(r: Resolved<T>): r is { state: "Known"; value: T } {
  return r.state === "Known";
}

export function unknownReason<T>(r: Resolved<T>): string | null {
  return r.state === "Unknown" ? r.value.reason : null;
}

export type Branch = { Named: string } | { Detached: { commit: string } };

export type UpstreamBasis =
  | { Configured: { refname: string } }
  | { Inferred: { refname: string } }
  | "None";

export type AheadBehind = { basis: UpstreamBasis; ahead: number; behind: number };

export type PathCounts = { staged: number; unstaged: number; untracked: number; conflicted: number };

export type InProgressOperation =
  | "None"
  | "Merge"
  | "Rebase"
  | "RebaseInteractive"
  | "CherryPick"
  | "Revert"
  | "Bisect"
  | "AmSession";

export type RepositoryState = {
  root: string;
  branch: Resolved<Branch>;
  ahead_behind: Resolved<AheadBehind>;
  paths: Resolved<PathCounts>;
  in_progress: InProgressOperation;
  stash_count: Resolved<number>;
  submodule_count: Resolved<number>;
};

export type GitlinkDivergence =
  | "InSync"
  | { Diverged: { ahead: number; behind: number } }
  | "GitlinkObjectMissingLocally"
  | "UnrelatedHistories";

export type SubmoduleState = {
  name: string;
  path: string;
  declared_branch: string | null;
  url: string | null;
  initialized: boolean;
  gitlink_commit: Resolved<string>;
  branch: Resolved<Branch>;
  gitlink_divergence: Resolved<GitlinkDivergence>;
  remote_basis: Resolved<UpstreamBasis>;
  remote_ahead_behind: Resolved<[number, number]>;
  dirty: Resolved<boolean>;
  last_fetch_unix_secs: Resolved<number | null>;
};

export function branchLabel(b: Resolved<Branch>): string {
  if (!isKnown(b)) return `unknown (${unknownReason(b)})`;
  const branch = b.value;
  if ("Named" in branch) return branch.Named;
  return `detached @ ${branch.Detached.commit.slice(0, 7)}`;
}

export function upstreamBasisLabel(basis: UpstreamBasis): { label: string; inferred: boolean } {
  if (basis === "None") return { label: "no remote basis", inferred: false };
  if ("Configured" in basis) return { label: basis.Configured.refname, inferred: false };
  return { label: basis.Inferred.refname, inferred: true };
}

export type ChangeCode =
  | "Unmodified"
  | "Modified"
  | "TypeChanged"
  | "Added"
  | "Deleted"
  | "Renamed"
  | "Copied"
  | "Unmerged";

export type SubmoduleFlags = {
  commit_changed: boolean;
  has_tracked_changes: boolean;
  has_untracked_changes: boolean;
};

export type ChangedEntry = {
  path: string;
  staged: ChangeCode;
  unstaged: ChangeCode;
  submodule: SubmoduleFlags | null;
  rename_or_copy_from: [string, number] | null;
};

export type ConflictEntry = { path: string; code: string };

export type WorkingCopyStatus = {
  changed: ChangedEntry[];
  untracked: string[];
  conflicted: ConflictEntry[];
};

export type Hunk = {
  header: string;
  lines: string[];
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
};

export type NonTextualDiff =
  | { Binary: { old_size: number | null; new_size: number | null; old_sha: string | null; new_sha: string | null } }
  | { Submodule: { old_commit: string | null; new_commit: string | null } }
  | { Symlink: { old_target: string | null; new_target: string | null } }
  | { ModeOnly: { old_mode: string; new_mode: string } };

export type FileDiff = {
  header_lines: string[];
  hunks: Hunk[];
  is_binary: boolean;
  non_textual: NonTextualDiff | null;
  path?: string | null;
};

export function changeCodeLabel(c: ChangeCode): string {
  switch (c) {
    case "Modified":
      return "M";
    case "TypeChanged":
      return "T";
    case "Added":
      return "A";
    case "Deleted":
      return "D";
    case "Renamed":
      return "R";
    case "Copied":
      return "C";
    case "Unmerged":
      return "U";
    default:
      return "";
  }
}

export function gitlinkDivergenceLabel(d: Resolved<GitlinkDivergence>): string {
  if (!isKnown(d)) return `unknown (${unknownReason(d)})`;
  const v = d.value;
  if (v === "InSync") return "in sync";
  if (v === "GitlinkObjectMissingLocally") return "gitlink commit unknown locally";
  if (v === "UnrelatedHistories") return "unrelated histories";
  return `${v.Diverged.ahead} ahead / ${v.Diverged.behind} behind`;
}

export type IgnoreTarget = "GitIgnore" | "GitInfoExclude";

export interface IgnoreExplanation {
  source: string;
  line_number: number;
  pattern: string;
  path: string;
}
