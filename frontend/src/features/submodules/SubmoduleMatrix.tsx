import { type Component, For, Show, createMemo, createSignal } from "solid-js";
import {
  branchLabel,
  gitlinkDivergenceLabel,
  isKnown,
  unknownReason,
  upstreamBasisLabel,
  type MalformedGitmodulesEntry,
  type SubmoduleState,
} from "../../api/types";
import {
  type SortKey,
  type SortDirection,
  type StateFilter,
  isDirty,
  isDetached,
  gitlinkAheadBehind,
  hasNoRemoteBasis,
  hasDrift,
  isFetchStale,
  lastFetchLabel,
  submoduleDrifts,
} from "./submodule-helpers";
import "./submodule-matrix.css";

export {
  type SortKey,
  type SortDirection,
  type StateFilter,
  isDirty,
  isDetached,
  gitlinkAheadBehind,
  hasNoRemoteBasis,
  hasDrift,
  isFetchStale,
  lastFetchLabel,
  submoduleDrifts,
};

export const SubmoduleMatrix: Component<{
  submodules: SubmoduleState[];
  malformedEntries?: MalformedGitmodulesEntry[];
  selectedPaths?: Set<string>;
  onSelectionChange?: (selected: Set<string>) => void;
  onDrillIn: (path: string, name: string) => void;
}> = (props) => {
  const [filter, setFilter] = createSignal<StateFilter>("all");
  const [searchQuery, setSearchQuery] = createSignal("");
  const [sortKey, setSortKey] = createSignal<SortKey>("name");
  const [sortDirection, setSortDirection] = createSignal<SortDirection>("asc");
  const [internalSelected, setInternalSelected] = createSignal<Set<string>>(new Set());
  const [groupByBranch, setGroupByBranch] = createSignal(false);
  const [collapsedGroups, setCollapsedGroups] = createSignal<Set<string>>(new Set());
  const [lastClickedIndex, setLastClickedIndex] = createSignal<number | null>(null);

  const selected = () => props.selectedPaths ?? internalSelected();
  const updateSelected = (next: Set<string>) => {
    if (props.onSelectionChange) {
      props.onSelectionChange(next);
    } else {
      setInternalSelected(next);
    }
  };

  const handleSort = (key: SortKey) => {
    if (sortKey() === key) {
      setSortDirection((prev) => (prev === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDirection("asc");
    }
  };

  const filtered = createMemo(() => {
    const f = filter();
    const query = searchQuery().trim().toLowerCase();

    return props.submodules.filter((s) => {
      if (query && !s.name.toLowerCase().includes(query) && !s.relative_path.toLowerCase().includes(query)) {
        return false;
      }
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
        case "drifted":
          return hasDrift(s);
        case "stale-fetch":
          return isFetchStale(s);
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
    const dir = sortDirection() === "asc" ? 1 : -1;

    list.sort((a, b) => {
      let cmp = 0;
      switch (key) {
        case "name":
          cmp = a.name.localeCompare(b.name);
          break;
        case "branch":
          cmp = branchLabel(a.branch).localeCompare(branchLabel(b.branch));
          break;
        case "gitlink": {
          const da = gitlinkAheadBehind(a);
          const db = gitlinkAheadBehind(b);
          const valA = da ? da.ahead + da.behind : 0;
          const valB = db ? db.ahead + db.behind : 0;
          cmp = valA - valB;
          break;
        }
        case "remote": {
          const aAhead = isKnown(a.remote_ahead_behind) ? a.remote_ahead_behind.value[0] : 0;
          const bAhead = isKnown(b.remote_ahead_behind) ? b.remote_ahead_behind.value[0] : 0;
          cmp = aAhead - bAhead;
          break;
        }
        case "dirty":
          cmp = Number(isDirty(b)) - Number(isDirty(a));
          break;
        case "last_fetch": {
          const ta = isKnown(a.last_fetch_unix_secs) ? a.last_fetch_unix_secs.value ?? 0 : 0;
          const tb = isKnown(b.last_fetch_unix_secs) ? b.last_fetch_unix_secs.value ?? 0 : 0;
          cmp = ta - tb;
          break;
        }
      }
      return cmp * dir;
    });
    return list;
  });

  const groups = createMemo(() => {
    if (!groupByBranch()) return null;
    const map = new Map<string, SubmoduleState[]>();
    for (const s of sorted()) {
      const key = !s.initialized ? "Uninitialized" : branchLabel(s.branch);
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
      drifted: all.filter(hasDrift).length,
      staleFetch: all.filter(isFetchStale).length,
      noRemoteBasis: all.filter(hasNoRemoteBasis).length,
    };
  });

  function toggleSelected(path: string, event?: MouseEvent, index?: number) {
    const next = new Set(selected());
    const displayed = sorted();

    if (event?.shiftKey && lastClickedIndex() !== null && index !== undefined) {
      const start = Math.min(lastClickedIndex()!, index);
      const end = Math.max(lastClickedIndex()!, index);
      for (let i = start; i <= end; i++) {
        next.add(displayed[i].path);
      }
    } else {
      if (next.has(path)) next.delete(path);
      else next.add(path);
      if (index !== undefined) setLastClickedIndex(index);
    }
    updateSelected(next);
  }

  function selectAllDisplayed() {
    const next = new Set(selected());
    for (const s of sorted()) {
      next.add(s.path);
    }
    updateSelected(next);
  }

  function clearSelection() {
    updateSelected(new Set());
  }

  function toggleAllHeader() {
    const displayed = sorted();
    const allSelected = displayed.length > 0 && displayed.every((s) => selected().has(s.path));
    if (allSelected) {
      const next = new Set(selected());
      for (const s of displayed) next.delete(s.path);
      updateSelected(next);
    } else {
      selectAllDisplayed();
    }
  }

  function toggleGroupCollapse(groupName: string) {
    const next = new Set(collapsedGroups());
    if (next.has(groupName)) next.delete(groupName);
    else next.add(groupName);
    setCollapsedGroups(next);
  }

  function toggleGroupSelect(groupItems: SubmoduleState[]) {
    const next = new Set(selected());
    const allInGroup = groupItems.every((s) => next.has(s.path));
    if (allInGroup) {
      for (const s of groupItems) next.delete(s.path);
    } else {
      for (const s of groupItems) next.add(s.path);
    }
    updateSelected(next);
  }

  const isAllHeaderSelected = () => {
    const displayed = sorted();
    return displayed.length > 0 && displayed.every((s) => selected().has(s.path));
  };

  const isIndeterminate = () => {
    const displayed = sorted();
    const count = displayed.filter((s) => selected().has(s.path)).length;
    return count > 0 && count < displayed.length;
  };

  function SortHeader(props2: { label: string; field: SortKey }) {
    const active = () => sortKey() === props2.field;
    return (
      <th>
        <button
          type="button"
          class="th-sort"
          classList={{ "th-sort-active": active() }}
          onClick={() => handleSort(props2.field)}
          aria-label={`Sort by ${props2.label}`}
        >
          {props2.label}
          <span class="sort-indicator" aria-hidden="true">
            {active() ? (sortDirection() === "asc" ? " ▲" : " ▼") : ""}
          </span>
        </button>
      </th>
    );
  }

  function Row(props2: { s: SubmoduleState; index: number }) {
    const s = props2.s;
    const dirty = isDirty(s);
    const gitlink = gitlinkDivergenceLabel(s.gitlink_divergence);
    const basis = isKnown(s.remote_basis) ? upstreamBasisLabel(s.remote_basis.value) : null;
    const isSelected = () => selected().has(s.path);
    const stale = isFetchStale(s);

    return (
      <tr
        classList={{
          "row-dirty": dirty,
          "row-uninitialized": !s.initialized,
          "row-selected": isSelected(),
        }}
        onClick={(e) => {
          if ((e.target as HTMLElement).tagName !== "BUTTON" && (e.target as HTMLElement).tagName !== "INPUT") {
            toggleSelected(s.path, e, props2.index);
          }
        }}
      >
        <td class="col-checkbox">
          <input
            type="checkbox"
            checked={isSelected()}
            onChange={(e) => {
              e.stopPropagation();
              toggleSelected(s.path, undefined, props2.index);
            }}
            aria-label={`Select ${s.name}`}
          />
        </td>
        <td class="col-name">
          <span
            class="name-wrapper"
            classList={{ "nested-submodule": s.depth > 1 }}
            style={{ "--depth": String(s.depth - 1) }}
          >
            <button
              type="button"
              class="collapse-toggle"
              onClick={() => props.onDrillIn(s.path, s.name)}
              disabled={!s.initialized}
              title={s.initialized ? `Open submodule ${s.name}` : `Submodule ${s.name} is not initialized`}
            >
              {s.name}
            </button>
            <Show when={s.depth > 1}>
              <span class="badge badge-neutral" title={`Nested submodule (depth ${s.depth})`}>
                L{s.depth}
              </span>
            </Show>
            <For each={submoduleDrifts(s)}>
              {(d) => (
                <span
                  class="badge badge-warning"
                  title={
                    typeof d === "string"
                      ? d
                      : `UrlMismatch: ${d.UrlMismatch.declared_url} vs ${d.UrlMismatch.config_url}`
                  }
                >
                  {typeof d === "string"
                    ? d === "DeclaredButAbsent"
                      ? "absent"
                      : d === "PresentButUndeclared"
                      ? "undeclared"
                      : "orphaned"
                    : "url-mismatch"}
                </span>
              )}
            </For>
          </span>
        </td>
        <td class="col-branch">
          <span class="branch-label">{branchLabel(s.branch)}</span>
          <Show when={isDetached(s)}>
            <span class="badge badge-neutral" title="Detached HEAD">
              detached
            </span>
          </Show>
        </td>
        <td
          class="col-gitlink"
          classList={{
            "cell-diverged": gitlinkAheadBehind(s) !== null,
            "cell-in-sync": isKnown(s.gitlink_divergence) && s.gitlink_divergence.value === "InSync",
          }}
        >
          {gitlink}
        </td>
        <td class="col-remote">
          <Show
            when={basis}
            fallback={
              <span class="text-muted">
                {isKnown(s.remote_basis) ? "no basis" : `unknown (${unknownReason(s.remote_basis)})`}
              </span>
            }
          >
            {(b) => (
              <>
                <span class="divergence-counts">
                  {isKnown(s.remote_ahead_behind)
                    ? `${s.remote_ahead_behind.value[0]} ahead / ${s.remote_ahead_behind.value[1]} behind`
                    : "—"}
                </span>
                <span
                  class="text-faint"
                  title={b().inferred ? "Inferred: @{u} is not configured" : "Configured upstream"}
                >
                  {" "}
                  ({b().inferred ? "inferred" : "configured"}: {b().label})
                </span>
              </>
            )}
          </Show>
        </td>
        <td class="col-dirty" classList={{ "cell-dirty": dirty }}>
          {isKnown(s.dirty) ? (dirty ? "dirty" : "clean") : `unknown (${unknownReason(s.dirty)})`}
        </td>
        <td class="col-fetch" classList={{ "cell-stale": stale }}>
          <span>{lastFetchLabel(s)}</span>
          <Show when={stale}>
            <span class="badge badge-warning" title="Fetch stale (>1 day ago or never)">
              stale
            </span>
          </Show>
        </td>
      </tr>
    );
  }

  return (
    <div class="submodule-matrix">
      <div class="matrix-toolbar" role="toolbar" aria-label="Submodule matrix filters">
        <div class="filter-pills" role="radiogroup" aria-label="Filter submodules by state">
          <button
            type="button"
            class="pill-btn"
            classList={{ active: filter() === "all" }}
            onClick={() => setFilter("all")}
          >
            All <span class="pill-count">{counts().total}</span>
          </button>
          <button
            type="button"
            class="pill-btn"
            classList={{ active: filter() === "dirty" }}
            onClick={() => setFilter("dirty")}
          >
            Dirty <span class="pill-count">{counts().dirty}</span>
          </button>
          <button
            type="button"
            class="pill-btn"
            classList={{ active: filter() === "detached" }}
            onClick={() => setFilter("detached")}
          >
            Detached <span class="pill-count">{counts().detached}</span>
          </button>
          <button
            type="button"
            class="pill-btn"
            classList={{ active: filter() === "behind-gitlink" }}
            onClick={() => setFilter("behind-gitlink")}
          >
            Behind gitlink <span class="pill-count">{counts().behindGitlink}</span>
          </button>
          <button
            type="button"
            class="pill-btn"
            classList={{ active: filter() === "ahead-gitlink" }}
            onClick={() => setFilter("ahead-gitlink")}
          >
            Ahead of gitlink <span class="pill-count">{counts().aheadGitlink}</span>
          </button>
          <button
            type="button"
            class="pill-btn"
            classList={{ active: filter() === "uninitialized" }}
            onClick={() => setFilter("uninitialized")}
          >
            Uninitialized <span class="pill-count">{counts().uninitialized}</span>
          </button>
          <button
            type="button"
            class="pill-btn"
            classList={{ active: filter() === "drifted" }}
            onClick={() => setFilter("drifted")}
          >
            Drifted <span class="pill-count">{counts().drifted}</span>
          </button>
          <button
            type="button"
            class="pill-btn"
            classList={{ active: filter() === "stale-fetch" }}
            onClick={() => setFilter("stale-fetch")}
          >
            Stale fetch <span class="pill-count">{counts().staleFetch}</span>
          </button>
          <button
            type="button"
            class="pill-btn"
            classList={{ active: filter() === "no-remote-basis" }}
            onClick={() => setFilter("no-remote-basis")}
          >
            No basis <span class="pill-count">{counts().noRemoteBasis}</span>
          </button>
        </div>

        <div class="matrix-controls">
          <input
            type="search"
            class="matrix-search-input"
            placeholder="Search submodules…"
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.currentTarget.value)}
            aria-label="Search submodules by name or path"
          />

          <label class="group-toggle-label">
            <input
              type="checkbox"
              checked={groupByBranch()}
              onChange={(e) => setGroupByBranch(e.currentTarget.checked)}
            />
            Group by branch
          </label>
        </div>
      </div>

      <Show when={selected().size > 0}>
        <div class="matrix-selection-bar" role="region" aria-label="Selection actions">
          <span class="selection-text">
            <strong>{selected().size}</strong> submodule{selected().size === 1 ? "" : "s"} selected
          </span>
          <div class="selection-actions">
            <button type="button" class="btn-action-text" onClick={selectAllDisplayed}>
              Select all displayed ({sorted().length})
            </button>
            <button type="button" class="btn-action-text" onClick={clearSelection}>
              Clear selection
            </button>
          </div>
        </div>
      </Show>

      <Show when={props.malformedEntries && props.malformedEntries.length > 0}>
        <div class="matrix-malformed-alert" role="alert">
          <strong>Malformed .gitmodules entries:</strong>
          <ul>
            <For each={props.malformedEntries}>
              {(m) => (
                <li>
                  Line {m.line_number}: <code>{m.raw_text}</code> — {m.reason}
                </li>
              )}
            </For>
          </ul>
        </div>
      </Show>

      <div class="matrix-scroll">
        <table class="matrix-table">
          <thead>
            <tr>
              <th class="col-checkbox" aria-label="Select all submodules">
                <input
                  type="checkbox"
                  checked={isAllHeaderSelected()}
                  ref={(el) => {
                    el.indeterminate = isIndeterminate();
                  }}
                  onChange={toggleAllHeader}
                  aria-label="Select all submodules"
                />
              </th>
              <SortHeader label="Submodule" field="name" />
              <SortHeader label="Branch" field="branch" />
              <SortHeader label="Gitlink divergence" field="gitlink" />
              <SortHeader label="Remote divergence" field="remote" />
              <SortHeader label="Status" field="dirty" />
              <SortHeader label="Last fetch" field="last_fetch" />
            </tr>
          </thead>
          <tbody>
            <Show
              when={groupByBranch() && groups()}
              fallback={
                <For each={sorted()}>
                  {(s, i) => <Row s={s} index={i()} />}
                </For>
              }
            >
              <For each={Array.from(groups()!.entries())}>
                {([groupName, items]) => {
                  const isCollapsed = () => collapsedGroups().has(groupName);
                  const isGroupAllSelected = () => items.every((s) => selected().has(s.path));
                  return (
                    <>
                      <tr class="group-row">
                        <td class="col-checkbox">
                          <input
                            type="checkbox"
                            checked={isGroupAllSelected()}
                            onChange={() => toggleGroupSelect(items)}
                            aria-label={`Select all in branch group ${groupName}`}
                          />
                        </td>
                        <td colspan="6">
                          <button
                            type="button"
                            class="group-toggle-btn"
                            onClick={() => toggleGroupCollapse(groupName)}
                            aria-expanded={!isCollapsed()}
                          >
                            <span class="group-arrow">{isCollapsed() ? "▶" : "▼"}</span>
                            <span class="group-title">{groupName}</span>
                            <span class="group-badge">
                              {items.length} submodule{items.length === 1 ? "" : "s"}
                            </span>
                          </button>
                        </td>
                      </tr>
                      <Show when={!isCollapsed()}>
                        <For each={items}>
                          {(s, i) => <Row s={s} index={i()} />}
                        </For>
                      </Show>
                    </>
                  );
                }}
              </For>
            </Show>
          </tbody>
        </table>
      </div>
    </div>
  );
};
