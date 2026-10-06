import {
  type Component,
  For,
  Show,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import { listen } from "@tauri-apps/api/event";
import {
  branchLabel,
  gitlinkDivergenceLabel,
  isKnown,
  unknownReason,
  upstreamBasisLabel,
  type BulkCheckoutPreview,
  type BulkOperationResult,
  type BulkPullPreview,
  type BulkResetPreview,
  type MalformedGitmodulesEntry,
  type SubmoduleRefreshProgress,
  type SubmoduleState,
} from "../../api/types";
import {
  bumpBulkGitlinks,
  cancelSubmoduleNetworkRefresh,
  executeBulkCheckout,
  executeBulkPull,
  executeBulkReset,
  previewBulkCheckout,
  previewBulkPull,
  previewBulkReset,
  refreshSubmoduleNetwork,
} from "../../api/commands";
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
import { Icon } from "../../ui/Icon";

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
  root?: string;
  submodules: SubmoduleState[];
  malformedEntries?: MalformedGitmodulesEntry[];
  selectedPaths?: Set<string>;
  onSelectionChange?: (selected: Set<string>) => void;
  onDrillIn: (path: string, name: string) => void;
  onRefreshNeeded?: () => void;
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

  const [isRefreshing, setIsRefreshing] = createSignal(false);
  const [refreshProgressText, setRefreshProgressText] = createSignal("");
  const [activeBulkAction, setActiveBulkAction] = createSignal<
    "checkout" | "pull" | "reset" | "bump" | null
  >(null);
  const [targetBranch, setTargetBranch] = createSignal("");
  const [checkoutPreview, setCheckoutPreview] = createSignal<BulkCheckoutPreview | null>(null);
  const [pullPreview, setPullPreview] = createSignal<BulkPullPreview | null>(null);
  const [resetPreview, setResetPreview] = createSignal<BulkResetPreview | null>(null);
  const [bulkLoading, setBulkLoading] = createSignal(false);
  const [bulkOutcome, setBulkOutcome] = createSignal<BulkOperationResult | null>(null);
  const [gitlinkBumpWarning, setGitlinkBumpWarning] = createSignal<string | null>(null);
  const [allowUnpushedBump, setAllowUnpushedBump] = createSignal(false);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    listen<SubmoduleRefreshProgress>("submodule:refresh-progress", (evt) => {
      const payload = evt.payload;
      const stage = payload.stage;
      if (stage.stage === "fetching") {
        setRefreshProgressText(`Fetching ${payload.relative_path} (${stage.remote})…`);
      } else if (stage.stage === "failed") {
        setRefreshProgressText(`Failed ${payload.relative_path}: ${stage.error}`);
      } else if (stage.stage === "skipped") {
        setRefreshProgressText(`Skipped ${payload.relative_path}: ${stage.reason}`);
      }
    }).then((fn) => {
      unlisten = fn;
    });

    onCleanup(() => {
      if (unlisten) unlisten();
    });
  });

  async function handleRefreshNetwork() {
    if (!props.root || isRefreshing()) return;
    setIsRefreshing(true);
    setRefreshProgressText("Starting network refresh…");
    try {
      const res = await refreshSubmoduleNetwork(props.root, {
        concurrency: 8,
        prune: true,
      });
      setRefreshProgressText(
        `Done: ${res.succeeded} updated, ${res.skipped} skipped, ${res.failed} failed`
      );
      props.onRefreshNeeded?.();
    } catch (e) {
      setRefreshProgressText(`Refresh failed: ${e}`);
    } finally {
      setTimeout(() => {
        setIsRefreshing(false);
      }, 2500);
    }
  }

  async function handleCancelRefresh() {
    try {
      await cancelSubmoduleNetworkRefresh();
    } catch {}
  }

  function openBulkCheckout() {
    setActiveBulkAction("checkout");
    setTargetBranch("");
    setCheckoutPreview(null);
  }

  async function handlePreviewCheckout() {
    if (!props.root || !targetBranch().trim()) return;
    setBulkLoading(true);
    try {
      const paths = Array.from(selected());
      const prev = await previewBulkCheckout(props.root, targetBranch().trim(), paths);
      setCheckoutPreview(prev);
    } catch (e) {
      alert(`Preview failed: ${e}`);
    } finally {
      setBulkLoading(false);
    }
  }

  async function handleExecuteCheckout() {
    if (!props.root || !targetBranch().trim()) return;
    setBulkLoading(true);
    try {
      const paths = Array.from(selected());
      const res = await executeBulkCheckout(props.root, targetBranch().trim(), paths);
      setBulkOutcome(res);
      setActiveBulkAction(null);
      props.onRefreshNeeded?.();
    } catch (e) {
      alert(`Checkout failed: ${e}`);
    } finally {
      setBulkLoading(false);
    }
  }

  async function openBulkPull() {
    if (!props.root) return;
    setActiveBulkAction("pull");
    setBulkLoading(true);
    try {
      const paths = Array.from(selected());
      const prev = await previewBulkPull(props.root, paths);
      setPullPreview(prev);
    } catch (e) {
      alert(`Pull preview failed: ${e}`);
    } finally {
      setBulkLoading(false);
    }
  }

  async function handleExecutePull() {
    if (!props.root) return;
    setBulkLoading(true);
    try {
      const paths = Array.from(selected());
      const res = await executeBulkPull(props.root, { strategy: "merge", paths });
      setBulkOutcome(res);
      setActiveBulkAction(null);
      props.onRefreshNeeded?.();
    } catch (e) {
      alert(`Pull failed: ${e}`);
    } finally {
      setBulkLoading(false);
    }
  }

  async function openBulkReset() {
    if (!props.root) return;
    setActiveBulkAction("reset");
    setBulkLoading(true);
    try {
      const paths = Array.from(selected());
      const prev = await previewBulkReset(props.root, paths);
      setResetPreview(prev);
    } catch (e) {
      alert(`Reset preview failed: ${e}`);
    } finally {
      setBulkLoading(false);
    }
  }

  async function handleExecuteReset() {
    if (!props.root) return;
    setBulkLoading(true);
    try {
      const paths = Array.from(selected());
      const res = await executeBulkReset(props.root, paths);
      setBulkOutcome(res);
      setActiveBulkAction(null);
      props.onRefreshNeeded?.();
    } catch (e) {
      alert(`Reset failed: ${e}`);
    } finally {
      setBulkLoading(false);
    }
  }

  async function handleBumpGitlinks() {
    if (!props.root) return;
    setBulkLoading(true);
    try {
      const paths = Array.from(selected());
      const outcomes = await bumpBulkGitlinks(props.root, paths, allowUnpushedBump());
      const unpushed = outcomes.find((o) => o.status === "unpushed_refused");
      if (unpushed && unpushed.status === "unpushed_refused") {
        setGitlinkBumpWarning(unpushed.reason);
        setActiveBulkAction("bump");
        return;
      }
      const succeeded = outcomes.filter((o) => o.status === "success").length;
      const failed = outcomes.filter((o) => o.status === "failed").length;
      setBulkOutcome({
        total: outcomes.length,
        succeeded,
        skipped: 0,
        failed,
        items: outcomes.map((o) => ({
          path: o.submodule_path,
          relative_path: o.relative_path,
          outcome:
            o.status === "success"
              ? {
                  status: "success",
                  message: `Bumped to ${o.head_commit}${
                    o.warning ? ` (${o.warning})` : ""
                  }`,
                }
              : {
                  status: "failed",
                  error: o.status === "failed" ? o.error : "refused",
                },
        })),
      });
      setActiveBulkAction(null);
      setGitlinkBumpWarning(null);
      props.onRefreshNeeded?.();
    } catch (e) {
      alert(`Bump gitlinks failed: ${e}`);
    } finally {
      setBulkLoading(false);
    }
  }

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
    const remoteAheadBehind = isKnown(s.remote_ahead_behind) ? s.remote_ahead_behind.value : null;

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
                  <Icon name="warning" size={12} class="badge-icon" />
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
              <Icon name="branch" size={12} class="badge-icon" /> detached
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
          <span class="state-icon" aria-hidden="true">
            {!isKnown(s.gitlink_divergence) ? (
              <Icon name="warning" size={12} />
            ) : s.gitlink_divergence.value === "InSync" ? (
              <Icon name="check" size={12} />
            ) : typeof s.gitlink_divergence.value === "object" && "Ahead" in s.gitlink_divergence.value ? (
              <Icon name="arrow-up" size={12} />
            ) : typeof s.gitlink_divergence.value === "object" && "Behind" in s.gitlink_divergence.value ? (
              <Icon name="arrow-down" size={12} />
            ) : typeof s.gitlink_divergence.value === "object" && "Both" in s.gitlink_divergence.value ? (
              <Icon name="arrow-up-down" size={12} />
            ) : (
              <Icon name="warning" size={12} />
            )}
          </span>
          <span>{gitlink}</span>
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
                <Show when={remoteAheadBehind}>
                  {(ab) => (
                    <span class="state-icon" aria-hidden="true">
                      {ab()[0] > 0 && ab()[1] > 0 ? (
                        <Icon name="arrow-up-down" size={12} />
                      ) : ab()[0] > 0 ? (
                        <Icon name="arrow-up" size={12} />
                      ) : ab()[1] > 0 ? (
                        <Icon name="arrow-down" size={12} />
                      ) : (
                        <Icon name="check" size={12} />
                      )}
                    </span>
                  )}
                </Show>
                <span class="divergence-counts">
                  {remoteAheadBehind
                    ? `${remoteAheadBehind[0]} ahead / ${remoteAheadBehind[1]} behind`
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
          {isKnown(s.dirty) ? (
            <>
              <span class="state-icon" aria-hidden="true">
                {dirty ? <Icon name="dirty" size={10} /> : <Icon name="check" size={12} />}
              </span>
              <span>{dirty ? "dirty" : "clean"}</span>
            </>
          ) : (
            `unknown (${unknownReason(s.dirty)})`
          )}
        </td>
        <td class="col-fetch" classList={{ "cell-stale": stale }}>
          <span>{lastFetchLabel(s)}</span>
          <Show when={stale}>
            <span class="badge badge-warning" title="Fetch stale (>1 day ago or never)">
              <Icon name="refresh" size={12} class="badge-icon" /> stale
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

          <div class="matrix-refresh-controls">
            <Show when={isRefreshing()}>
              <span class="matrix-refresh-status" role="status">
                <span class="matrix-spinner" aria-hidden="true">⟳</span>
                <span>{refreshProgressText()}</span>
              </span>
              <button
                type="button"
                class="btn-matrix-action"
                onClick={handleCancelRefresh}
                title="Cancel network refresh"
              >
                Cancel
              </button>
            </Show>
            <Show when={!isRefreshing()}>
              <button
                type="button"
                class="btn-matrix-action"
                onClick={handleRefreshNetwork}
                title="Fetch all submodules in parallel"
              >
                <span>⟳</span> Refresh Network
              </button>
            </Show>
          </div>
        </div>
      </div>

      <Show when={selected().size > 0}>
        <div class="matrix-selection-bar" role="region" aria-label="Selection actions">
          <span class="selection-text">
            <strong>{selected().size}</strong> submodule{selected().size === 1 ? "" : "s"} selected
          </span>
          <div class="selection-actions">
            <button
              type="button"
              class="btn-matrix-action"
              classList={{ "btn-matrix-action-primary": activeBulkAction() === "checkout" }}
              onClick={openBulkCheckout}
            >
              Checkout…
            </button>
            <button
              type="button"
              class="btn-matrix-action"
              classList={{ "btn-matrix-action-primary": activeBulkAction() === "pull" }}
              onClick={openBulkPull}
            >
              Pull
            </button>
            <button
              type="button"
              class="btn-matrix-action"
              classList={{ "btn-matrix-action-primary": activeBulkAction() === "reset" }}
              onClick={openBulkReset}
            >
              Reset to Gitlink
            </button>
            <button
              type="button"
              class="btn-matrix-action"
              classList={{ "btn-matrix-action-primary": activeBulkAction() === "bump" }}
              onClick={handleBumpGitlinks}
            >
              Bump Gitlink
            </button>
            <button type="button" class="btn-action-text" onClick={selectAllDisplayed}>
              Select all displayed ({sorted().length})
            </button>
            <button type="button" class="btn-action-text" onClick={clearSelection}>
              Clear selection
            </button>
          </div>
        </div>

        <Show when={activeBulkAction() === "checkout"}>
          <div class="bulk-action-panel" role="region" aria-label="Bulk checkout">
            <div class="bulk-action-row">
              <strong>Bulk Branch Checkout:</strong>
              <input
                type="text"
                class="matrix-text-input"
                placeholder="Target branch name…"
                value={targetBranch()}
                onInput={(e) => setTargetBranch(e.currentTarget.value)}
                onKeyDown={(e) => e.key === "Enter" && handlePreviewCheckout()}
              />
              <button
                type="button"
                class="btn-matrix-action"
                onClick={handlePreviewCheckout}
                disabled={!targetBranch().trim() || bulkLoading()}
              >
                Preview
              </button>
              <button
                type="button"
                class="btn-matrix-action btn-matrix-action-primary"
                onClick={handleExecuteCheckout}
                disabled={!checkoutPreview() || checkoutPreview()!.will_switch === 0 || bulkLoading()}
              >
                {bulkLoading() ? "Switching…" : `Switch ${checkoutPreview()?.will_switch ?? 0} Submodule(s)`}
              </button>
              <button
                type="button"
                class="btn-action-text"
                onClick={() => setActiveBulkAction(null)}
              >
                Close
              </button>
            </div>
            <Show when={checkoutPreview()}>
              {(prev) => (
                <div class="bulk-preview-badges">
                  <span class="badge">Will switch: {prev().will_switch}</span>
                  <span class="badge">Already on branch: {prev().already_on_branch}</span>
                  <span class="badge badge-warning">Skipped dirty: {prev().skipped_dirty}</span>
                  <span class="badge">Missing branch: {prev().missing_branch}</span>
                </div>
              )}
            </Show>
          </div>
        </Show>

        <Show when={activeBulkAction() === "pull"}>
          <div class="bulk-action-panel" role="region" aria-label="Bulk pull">
            <div class="bulk-action-row">
              <strong>Bulk Pull from Upstream:</strong>
              <Show when={pullPreview()}>
                {(prev) => (
                  <div class="bulk-preview-badges">
                    <span class="badge">Will pull: {prev().will_pull}</span>
                    <span class="badge">Already up to date: {prev().already_up_to_date}</span>
                    <span class="badge badge-warning">Skipped dirty: {prev().skipped_dirty}</span>
                    <span class="badge">No upstream: {prev().skipped_no_upstream}</span>
                  </div>
                )}
              </Show>
              <button
                type="button"
                class="btn-matrix-action btn-matrix-action-primary"
                onClick={handleExecutePull}
                disabled={bulkLoading() || (pullPreview() !== null && pullPreview()!.will_pull === 0)}
              >
                {bulkLoading() ? "Pulling…" : `Confirm Pull (${pullPreview()?.will_pull ?? 0})`}
              </button>
              <button
                type="button"
                class="btn-action-text"
                onClick={() => setActiveBulkAction(null)}
              >
                Close
              </button>
            </div>
          </div>
        </Show>

        <Show when={activeBulkAction() === "reset"}>
          <div class="bulk-action-panel" role="region" aria-label="Bulk reset to gitlink">
            <div class="bulk-action-row">
              <strong>Bulk Reset to Superproject Gitlink:</strong>
              <Show when={resetPreview()}>
                {(prev) => (
                  <div class="bulk-preview-badges">
                    <span class="badge">Will reset: {prev().will_reset}</span>
                    <span class="badge">Already in sync: {prev().already_in_sync}</span>
                    <span class="badge badge-warning">Skipped dirty: {prev().skipped_dirty}</span>
                  </div>
                )}
              </Show>
              <button
                type="button"
                class="btn-matrix-action btn-matrix-action-primary"
                onClick={handleExecuteReset}
                disabled={bulkLoading() || (resetPreview() !== null && resetPreview()!.will_reset === 0)}
              >
                {bulkLoading() ? "Resetting…" : `Confirm Reset (${resetPreview()?.will_reset ?? 0})`}
              </button>
              <button
                type="button"
                class="btn-action-text"
                onClick={() => setActiveBulkAction(null)}
              >
                Close
              </button>
            </div>
            <Show when={resetPreview() && resetPreview()!.skipped_dirty > 0}>
              <small class="text-muted">
                Note: {resetPreview()!.skipped_dirty} dirty submodule(s) will be refused to protect uncommitted changes.
              </small>
            </Show>
          </div>
        </Show>

        <Show when={activeBulkAction() === "bump" && gitlinkBumpWarning()}>
          <div class="bulk-action-panel matrix-malformed-alert" role="alert">
            <div class="bulk-action-row">
              <strong>Unpushed Commit Warning:</strong>
              <span>{gitlinkBumpWarning()}</span>
              <label class="group-toggle-label">
                <input
                  type="checkbox"
                  checked={allowUnpushedBump()}
                  onChange={(e) => setAllowUnpushedBump(e.currentTarget.checked)}
                />
                Allow bumping unpushed commit
              </label>
              <button
                type="button"
                class="btn-matrix-action btn-matrix-action-primary"
                disabled={!allowUnpushedBump() || bulkLoading()}
                onClick={handleBumpGitlinks}
              >
                Confirm Bump
              </button>
              <button
                type="button"
                class="btn-action-text"
                onClick={() => {
                  setActiveBulkAction(null);
                  setGitlinkBumpWarning(null);
                }}
              >
                Cancel
              </button>
            </div>
          </div>
        </Show>
      </Show>

      <Show when={bulkOutcome()}>
        {(outcome) => (
          <div class="matrix-outcome-banner" role="region" aria-label="Bulk operation results">
            <div class="matrix-outcome-banner-header">
              <span>
                Operation Results: <strong>{outcome().succeeded}</strong> succeeded,{" "}
                <strong>{outcome().skipped}</strong> skipped,{" "}
                <strong>{outcome().failed}</strong> failed (out of {outcome().total})
              </span>
              <button
                type="button"
                class="btn-action-text"
                onClick={() => setBulkOutcome(null)}
              >
                Dismiss
              </button>
            </div>
            <Show when={outcome().skipped > 0 || outcome().failed > 0}>
              <ul class="matrix-outcome-details">
                <For each={outcome().items.filter((i) => i.outcome.status !== "success")}>
                  {(item) => (
                    <li>
                      <strong>{item.relative_path}:</strong>{" "}
                      {item.outcome.status === "skipped"
                        ? `Skipped (${item.outcome.reason})`
                        : item.outcome.status === "failed"
                        ? `Failed (${item.outcome.error})`
                        : "Cancelled"}
                    </li>
                  )}
                </For>
              </ul>
            </Show>
          </div>
        )}
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
