import {
  isKnown,
  unknownReason,
  type SubmoduleDrift,
  type SubmoduleState,
} from "../../api/types";

export type SortKey = "name" | "branch" | "gitlink" | "remote" | "dirty" | "last_fetch";
export type SortDirection = "asc" | "desc";
export type StateFilter =
  | "all"
  | "dirty"
  | "detached"
  | "behind-gitlink"
  | "ahead-gitlink"
  | "uninitialized"
  | "drifted"
  | "stale-fetch"
  | "no-remote-basis";

export function isDirty(s: SubmoduleState): boolean {
  return isKnown(s.dirty) && s.dirty.value === true;
}

export function isDetached(s: SubmoduleState): boolean {
  return isKnown(s.branch) && "Detached" in s.branch.value;
}

export function gitlinkAheadBehind(s: SubmoduleState): { ahead: number; behind: number } | null {
  if (!isKnown(s.gitlink_divergence)) return null;
  const v = s.gitlink_divergence.value;
  if (typeof v === "object") {
    if ("Ahead" in v) return { ahead: v.Ahead.ahead, behind: 0 };
    if ("Behind" in v) return { ahead: 0, behind: v.Behind.behind };
    if ("Both" in v) return { ahead: v.Both.ahead, behind: v.Both.behind };
  }
  return null;
}

export function hasNoRemoteBasis(s: SubmoduleState): boolean {
  return isKnown(s.remote_basis) && s.remote_basis.value === "None";
}

export function hasDrift(s: SubmoduleState): boolean {
  return isKnown(s.drift) && s.drift.value.length > 0;
}

export function isFetchStale(s: SubmoduleState): boolean {
  if (!isKnown(s.last_fetch_unix_secs)) return false;
  const secs = s.last_fetch_unix_secs.value;
  if (secs === null) return true;
  const ageSeconds = Date.now() / 1000 - secs;
  return ageSeconds > 86400;
}

export function lastFetchLabel(s: SubmoduleState): string {
  if (!isKnown(s.last_fetch_unix_secs)) return `unknown (${unknownReason(s.last_fetch_unix_secs)})`;
  const secs = s.last_fetch_unix_secs.value;
  if (secs === null) return "never";
  const ageSeconds = Date.now() / 1000 - secs;
  const days = Math.floor(ageSeconds / 86400);
  if (days < 1) return "today";
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  if (months < 24) return `${months}mo ago`;
  return `${Math.floor(months / 12)}y ago`;
}

export function submoduleDrifts(s: SubmoduleState): SubmoduleDrift[] {
  return isKnown(s.drift) ? s.drift.value : [];
}
