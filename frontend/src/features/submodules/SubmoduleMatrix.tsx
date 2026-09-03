import { type Component, For, Show, createMemo, createSignal } from "solid-js";
import {
  branchLabel,
  gitlinkDivergenceLabel,
  isKnown,
  unknownReason,
  upstreamBasisLabel,
  type SubmoduleState,
} from "../../api/types";

type SortKey = "name" | "branch" | "gitlink" | "remote" | "dirty";
type StateFilter = "all" | "dirty" | "detached" | "behind-gitlink" | "ahead-gitlink" | "uninitialized" | "no-remote-basis";

function isDirty(s: SubmoduleState): boolean {
  return isKnown(s.dirty) && s.dirty.value === true;
}
function isDetached(s: SubmoduleState): boolean {
  return isKnown(s.branch) && "Detached" in s.branch.value;
}
function gitlinkAheadBehind(s: SubmoduleState): { ahead: number; behind: number } | null {
  if (!isKnown(s.gitlink_divergence)) return null;
  const v = s.gitlink_divergence.value;
  if (typeof v === "object" && "Diverged" in v) return v.Diverged;
  return null;
}
function hasNoRemoteBasis(s: SubmoduleState): boolean {
  return isKnown(s.remote_basis) && s.remote_basis.value === "None";
}
function lastFetchLabel(s: SubmoduleState): string {
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

export const SubmoduleMatrix: Component<{ submodules: SubmoduleState[]; onDrillIn: (path: string, name: string) => void }> = (
  props,
) => {
  const [filter, setFilter] = createSignal<StateFilter>("all");
  const [sortKey, setSortKey] = createSignal<SortKey>("name");
  const [selected, setSelected] = createSignal<Set<string>>(new Set());
  const [groupByBranch, setGroupByBranch] = createSignal(false);

  const filtered = createMemo(() => {
    const f = filter();
    return props.submodules.filter((s) => {
      switch (f) {
        case "dirty":
          return isDirty(s);
        case "detached":
          return isDetached(s);
        case "behind-gitlink": {
          const d = gitlinkAheadBehind(s);
          return d !== null && d.behind > 0;
        }
        case "ahead-gitlink": {
          const d = gitlinkAheadBehind(s);
          return d !== null && d.ahead > 0;
        }
        case "uninitialized":
          return !s.initialized;
        case "no-remote-basis":
          return hasNoRemoteBasis(s);
        default:
          return true;
      }
    });
  });

  const sorted = createMemo(() => {
    const list = [...filtered()];
    const key = sortKey();
    list.sort((a, b) => {
      if (key === "name") return a.name.localeCompare(b.name);
      if (key === "branch") return branchLabel(a.branch).localeCompare(branchLabel(b.branch));
      if (key === "dirty") return Number(isDirty(b)) - Number(isDirty(a));
      return 0;
    });
    return list;
  });

  const grouped = createMemo(() => {
    if (!groupByBranch()) return null;
    const map = new Map<string, SubmoduleState[]>();
    for (const s of sorted()) {
      const key = branchLabel(s.branch);
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(s);
    }
    return map;
  });

  const counts = createMemo(() => {
    const all = props.submodules;
    return {
      total: all.length,
      dirty: all.filter(isDirty).length,
      detached: all.filter(isDetached).length,
      behindGitlink: all.filter((s) => (gitlinkAheadBehind(s)?.behind ?? 0) > 0).length,
      aheadGitlink: all.filter((s) => (gitlinkAheadBehind(s)?.ahead ?? 0) > 0).length,
      uninitialized: all.filter((s) => !s.initialized).length,
      noRemoteBasis: all.filter(hasNoRemoteBasis).length,
    };
  });

  function toggleSelected(path: string) {
    const next = new Set(selected());
    if (next.has(path)) next.delete(path);
    else next.add(path);
    setSelected(next);
  }

  function Row(props2: { s: SubmoduleState }) {
    const s = props2.s;
    const dirty = isDirty(s);
    const gitlink = gitlinkDivergenceLabel(s.gitlink_divergence);
    const basis = isKnown(s.remote_basis) ? upstreamBasisLabel(s.remote_basis.value) : null;
    return (
      <tr classList={{ "row-dirty": dirty, "row-uninitialized": !s.initialized }}>
        <td>
          <input
            type="checkbox"
            checked={selected().has(s.path)}
            onChange={() => toggleSelected(s.path)}
            aria-label={`Select ${s.name}`}
          />
        </td>
        <td class="col-name">
          <button class="collapse-toggle" onClick={() => props.onDrillIn(s.path, s.name)} disabled={!s.initialized}>
            {s.name}
          </button>
        </td>
        <td>
          {branchLabel(s.branch)}
          <Show when={isDetached(s)}>
            <span class="badge badge-neutral" title="Detached HEAD">detached</span>
          </Show>
        </td>
        <td classList={{ "cell-diverged": gitlinkAheadBehind(s) !== null, "cell-in-sync": isKnown(s.gitlink_divergence) && s.gitlink_divergence.value === "InSync" }}>
          {gitlink}
        </td>
        <td>
          <Show when={basis} fallback={<span class="text-muted">{isKnown(s.remote_basis) ? "no basis" : `unknown (${unknownReason(s.remote_basis)})`}</span>}>
            {(b) => (
              <>
                {isKnown(s.remote_ahead_behind) ? `${s.remote_ahead_behind.value[0]} ahead / ${s.remote_ahead_behind.value[1]} behind` : "—"}
                <span class="text-faint" title={b().inferred ? "Inferred: @{u} is not configured" : "Configured upstream"}>
                  {" "}
                  ({b().inferred ? "inferred" : "configured"}: {b().label})
                </span>
              </>
            )}
          </Show>
        </td>
        <td classList={{ "cell-dirty": dirty }}>{isKnown(s.dirty) ? (dirty ? "dirty" : "clean") : `unknown (${unknownReason(s.dirty)})`}</td>
        <td>{lastFetchLabel(s)}</td>
      </tr>
    );
  }

  return (
    <div class="submodule-matrix">
      <div class="matrix-toolbar" role="toolbar" aria-label="Submodule matrix filters">
        <span class="matrix-counts">
          {counts().total} submodules · {counts().dirty} dirty · {counts().detached} detached ·{" "}
          {counts().behindGitlink} behind gitlink · {counts().aheadGitlink} ahead of gitlink ·{" "}
          {counts().uninitialized} uninitialized · {counts().noRemoteBasis} without remote basis
        </span>
        <label>
          Filter:
          <select value={filter()} onChange={(e) => setFilter(e.currentTarget.value as StateFilter)}>
            <option value="all">All</option>
            <option value="dirty">Dirty</option>
            <option value="detached">Detached</option>
            <option value="behind-gitlink">Behind gitlink</option>
            <option value="ahead-gitlink">Ahead of gitlink</option>
            <option value="uninitialized">Uninitialized</option>
            <option value="no-remote-basis">No remote basis</option>
          </select>
        </label>
        <label>
          <input type="checkbox" checked={groupByBranch()} onChange={(e) => setGroupByBranch(e.currentTarget.checked)} />
          Group by branch
        </label>
      </div>

      <div class="matrix-scroll">
        <table class="matrix-table">
          <thead>
            <tr>
              <th aria-label="Select" />
              <th>
                <button class="th-sort" onClick={() => setSortKey("name")}>Submodule</button>
              </th>
              <th>
                <button class="th-sort" onClick={() => setSortKey("branch")}>Branch</button>
              </th>
              <th>Gitlink divergence</th>
              <th>Remote divergence</th>
              <th>
                <button class="th-sort" onClick={() => setSortKey("dirty")}>Dirty</button>
              </th>
              <th>Last fetch</th>
            </tr>
          </thead>
          <tbody>
            <Show
              when={!grouped()}
              fallback={
                <For each={[...(grouped()?.entries() ?? [])]}>
                  {([branch, rows]) => (
                    <>
                      <tr class="group-row">
                        <td colSpan={7}>{branch} ({rows.length})</td>
                      </tr>
                      <For each={rows}>{(s) => <Row s={s} />}</For>
                    </>
                  )}
                </For>
              }
            >
              <For each={sorted()}>{(s) => <Row s={s} />}</For>
            </Show>
          </tbody>
        </table>
      </div>
    </div>
  );
};
