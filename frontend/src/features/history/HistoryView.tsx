import { type Component, For, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import {
  cancelHistorySearch,
  getHistoryCount,
  getHistoryGraph,
  getHistoryPage,
  searchHistory,
  type CommitSummary,
  type ContentSearchMode,
  type GraphRow,
  type HistoryScope,
  type HistorySearchResult,
} from "../../api/commands";
import { CommitDetailView } from "./CommitDetailView";
import { HistoricalFileView } from "./HistoricalFileView";
import { RevisionDiffView } from "./RevisionDiffView";
import { BlameView } from "../working-copy/BlameView";
import { InteractiveRebaseModal } from "../integration/InteractiveRebaseModal";
import { CherryPickModal } from "../integration/CherryPickModal";
import { RevertModal } from "../integration/RevertModal";

const ROW_HEIGHT = 24;
const PAGE_SIZE = 100;
const OVERSCAN = 20;
const MAX_CACHED_ROWS = 2000;
const LANE_WIDTH = 14;
const LANE_BUDGET = 12;
const GRAPH_WIDTH = LANE_BUDGET * LANE_WIDTH + LANE_WIDTH;

function pageIndexOf(rowIndex: number): number {
  return Math.floor(rowIndex / PAGE_SIZE);
}

export const HistoryView: Component<{
  root: string;
  hasUncommittedChanges: boolean;
  onSelectUncommitted: () => void;
  initialScope?: HistoryScope;
}> = (props) => {
  const [scope, setScope] = createSignal<HistoryScope>(props.initialScope ?? { kind: "CurrentBranch" });
  const [pathFilter, setPathFilter] = createSignal(
    props.initialScope && "path" in props.initialScope ? props.initialScope.path : "",
  );
  const [totalCount, setTotalCount] = createSignal(0);
  const [scrollTop, setScrollTop] = createSignal(0);
  const [viewportHeight, setViewportHeight] = createSignal(400);
  const [version, setVersion] = createSignal(0);
  const [graphVersion, setGraphVersion] = createSignal(0);
  const [error, setError] = createSignal<string | null>(null);
  const [selectedSha, setSelectedSha] = createSignal<string | null>(null);
  const [compareSha, setCompareSha] = createSignal<string | null>(null);
  const [compareMode, setCompareMode] = createSignal(false);
  const [historicalFile, setHistoricalFile] = createSignal<{ path: string; rev: string } | null>(null);
  const [blameFile, setBlameFile] = createSignal<{ path: string; rev?: string } | null>(null);
  const [searchOpen, setSearchOpen] = createSignal(false);
  const [searchQuery, setSearchQuery] = createSignal("");
  const [searchType, setSearchType] = createSignal<"message" | "author" | "path" | "content_pickaxe" | "content_regex">("message");
  const [searchSince, setSearchSince] = createSignal("");
  const [searchUntil, setSearchUntil] = createSignal("");
  const [isSearching, setIsSearching] = createSignal(false);
  const [searchResults, setSearchResults] = createSignal<HistorySearchResult | null>(null);
  const [searchError, setSearchError] = createSignal<string | null>(null);
  const [rebaseTarget, setRebaseTarget] = createSignal<{ baseSha: string; subject: string } | null>(null);
  const [cherryPickTarget, setCherryPickTarget] = createSignal<{ sha: string; subject: string } | null>(null);
  const [revertTarget, setRevertTarget] = createSignal<{ sha: string; subject: string } | null>(null);
  const [integrationNotice, setIntegrationNotice] = createSignal<string | null>(null);

  async function executeSearch(loadMore = false) {
    const q = searchQuery().trim();
    const type = searchType();
    const since = searchSince().trim() || undefined;
    const until = searchUntil().trim() || undefined;

    let message: string | undefined;
    let author: string | undefined;
    let path: string | undefined;
    let content_query: string | undefined;
    let content_mode: ContentSearchMode | undefined;

    if (q) {
      switch (type) {
        case "message":
          message = q;
          break;
        case "author":
          author = q;
          break;
        case "path":
          path = q;
          break;
        case "content_pickaxe":
          content_query = q;
          content_mode = "Pickaxe";
          break;
        case "content_regex":
          content_query = q;
          content_mode = "Regex";
          break;
      }
    }

    if (!message && !author && !path && !content_query && !since && !until) {
      setSearchResults(null);
      setSearchError(null);
      return;
    }

    const currentCommits = loadMore ? (searchResults()?.commits ?? []) : [];
    const skip = currentCommits.length;

    setIsSearching(true);
    setSearchError(null);
    try {
      const result = await searchHistory(props.root, {
        scope: activeScope(),
        message,
        author,
        path,
        since,
        until,
        content_query,
        content_mode,
        skip,
        limit: 100,
      });
      if (loadMore) {
        setSearchResults({
          commits: [...currentCommits, ...result.commits],
          truncated: result.truncated,
        });
      } else {
        setSearchResults(result);
      }
    } catch (err) {
      const str = String(err);
      if (!str.includes("cancelled")) {
        setSearchError(str);
      }
    } finally {
      setIsSearching(false);
    }
  }

  async function handleCancelSearch() {
    try {
      await cancelHistorySearch();
    } catch (_) {}
    setIsSearching(false);
  }

  function handleClearSearch() {
    setSearchQuery("");
    setSearchSince("");
    setSearchUntil("");
    setSearchResults(null);
    setSearchError(null);
  }

  let cache = new Map<number, CommitSummary>();
  let loadedPages = new Set<number>();
  let loadingPages = new Set<number>();
  let graphRows: GraphRow[] = [];
  let shaIndex = new Map<string, number>();
  let containerEl: HTMLDivElement | undefined;
  let canvasEl: HTMLCanvasElement | undefined;

  createEffect(() => {
    if (props.initialScope) {
      setScope(props.initialScope);
      if ("path" in props.initialScope) {
        setPathFilter(props.initialScope.path);
      }
      setSelectedSha(null);
      setCompareSha(null);
      setHistoricalFile(null);
      setBlameFile(null);
    }
  });

  function activeScope(): HistoryScope {
    const s = scope();
    return s.kind === "Path" ? { kind: "Path", path: pathFilter() } : s;
  }

  function resetCache() {
    cache = new Map();
    loadedPages = new Set();
    loadingPages = new Set();
    graphRows = [];
    shaIndex = new Map();
    setVersion((v) => v + 1);
    setGraphVersion((v) => v + 1);
  }

  async function refetchCount() {
    try {
      const count = await getHistoryCount(props.root, activeScope());
      setTotalCount(count);
      setError(null);
    } catch (err) {
      setError(String(err));
      setTotalCount(0);
    }
  }

  async function refetchGraph() {
    const requestScope = activeScope();
    try {
      const result = await getHistoryGraph(props.root, requestScope);
      if (JSON.stringify(requestScope) !== JSON.stringify(activeScope())) return;
      graphRows = result.rows;
      shaIndex = new Map(graphRows.map((row, i) => [row.sha, i]));
      setGraphVersion((v) => v + 1);
    } catch (err) {
      setError(String(err));
    }
  }

  async function ensurePageLoaded(pageIndex: number) {
    if (loadedPages.has(pageIndex) || loadingPages.has(pageIndex)) return;
    loadingPages.add(pageIndex);
    const requestScope = activeScope();
    try {
      const rows = await getHistoryPage(props.root, requestScope, pageIndex * PAGE_SIZE, PAGE_SIZE);
      if (JSON.stringify(requestScope) !== JSON.stringify(activeScope())) {
        return; // scope changed while this page was in flight; discard the stale response
      }
      rows.forEach((row, i) => cache.set(pageIndex * PAGE_SIZE + i, row));
      loadedPages.add(pageIndex);
      evictFarEntries();
      setVersion((v) => v + 1);
    } catch (err) {
      setError(String(err));
    } finally {
      loadingPages.delete(pageIndex);
    }
  }

  function evictFarEntries() {
    if (cache.size <= MAX_CACHED_ROWS) return;
    const center = Math.floor(scrollTop() / ROW_HEIGHT);
    const halfWindow = MAX_CACHED_ROWS / 2;
    for (const index of [...cache.keys()]) {
      if (Math.abs(index - center) > halfWindow) {
        cache.delete(index);
        loadedPages.delete(pageIndexOf(index));
      }
    }
  }

  function visibleRange() {
    const start = Math.max(0, Math.floor(scrollTop() / ROW_HEIGHT) - OVERSCAN);
    const end = Math.min(
      totalCount(),
      Math.ceil((scrollTop() + viewportHeight()) / ROW_HEIGHT) + OVERSCAN,
    );
    return { start, end };
  }

  createEffect(() => {
    version();
    const { start, end } = visibleRange();
    const firstPage = pageIndexOf(start);
    const lastPage = pageIndexOf(Math.max(start, end - 1));
    for (let p = firstPage; p <= lastPage; p++) {
      void ensurePageLoaded(p);
    }
  });

  createEffect(() => {
    void props.root;
    void scope();
    void pathFilter();
    resetCache();
    setScrollTop(0);
    if (containerEl) containerEl.scrollTop = 0;
    void refetchCount();
    void refetchGraph();
  });

  onMount(() => {
    if (!containerEl) return;
    const observer = new ResizeObserver(() => {
      if (containerEl) setViewportHeight(containerEl.clientHeight);
    });
    observer.observe(containerEl);
    onCleanup(() => observer.disconnect());
  });

  function laneCenterX(lane: number): number {
    return LANE_WIDTH / 2 + lane * LANE_WIDTH;
  }

  createEffect(() => {
    graphVersion();
    version();
    scrollTop();
    const canvas = canvasEl;
    if (!canvas) return;
    const { start, end } = visibleRange();
    const height = Math.max(0, (end - start) * ROW_HEIGHT);
    canvas.style.top = `${start * ROW_HEIGHT}px`;
    canvas.width = GRAPH_WIDTH;
    canvas.height = height;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.strokeStyle = "currentColor";
    ctx.fillStyle = "currentColor";
    ctx.lineWidth = 1.5;

    for (let i = start; i < end; i++) {
      const row = graphRows[i];
      if (!row) continue;
      const y = (i - start) * ROW_HEIGHT + ROW_HEIGHT / 2;
      const x = laneCenterX(row.lane);

      for (const parentSha of row.parents) {
        const parentIndex = shaIndex.get(parentSha);
        if (parentIndex === undefined) continue;
        const parentRow = graphRows[parentIndex];
        const parentY = (parentIndex - start) * ROW_HEIGHT + ROW_HEIGHT / 2;
        const parentX = laneCenterX(parentRow.lane);
        ctx.beginPath();
        ctx.moveTo(x, y);
        ctx.lineTo(parentX, parentY);
        ctx.stroke();
      }

      ctx.beginPath();
      if (row.is_merge) {
        ctx.rect(x - 4, y - 4, 8, 8);
        ctx.fill();
      } else {
        ctx.arc(x, y, 3.5, 0, Math.PI * 2);
        ctx.fill();
      }
      if (row.overflow) {
        ctx.font = "9px sans-serif";
        ctx.fillText("+", x + 6, y + 3);
      }
    }
  });

  function rowsToRender() {
    version();
    const { start, end } = visibleRange();
    const rows: { index: number; commit: CommitSummary | undefined }[] = [];
    for (let i = start; i < end; i++) {
      rows.push({ index: i, commit: cache.get(i) });
    }
    return rows;
  }

  function onRowClick(e: MouseEvent, sha: string) {
    setHistoricalFile(null);
    setBlameFile(null);
    if (e.ctrlKey || e.metaKey || compareMode()) {
      if (!selectedSha()) {
        setSelectedSha(sha);
      } else if (selectedSha() === sha) {
        if (compareSha()) setCompareSha(null);
      } else {
        setCompareSha(sha);
      }
    } else {
      setSelectedSha(sha);
      setCompareSha(null);
    }
  }

  return (
    <div class="history-view">
      <div class="history-scope-bar">
        <select
          class="history-scope-select"
          value={scope().kind}
          onChange={(e) => {
            const val = e.currentTarget.value;
            setScope(val === "Path" ? { kind: "Path", path: pathFilter() } : ({ kind: val } as HistoryScope));
            setSelectedSha(null);
            setCompareSha(null);
            setHistoricalFile(null);
            setBlameFile(null);
          }}
        >
          <option value="CurrentBranch">Current branch</option>
          <option value="AllBranches">All branches</option>
          <option value="AllRefs">All refs</option>
          <option value="Path">Path</option>
        </select>
        <Show when={scope().kind === "Path"}>
          <input
            class="history-path-input"
            type="text"
            placeholder="path/to/file or directory"
            value={pathFilter()}
            onInput={(e) => setPathFilter(e.currentTarget.value)}
          />
          <Show when={pathFilter().trim()}>
            <button
              class="collapse-toggle"
              onClick={() => setBlameFile({ path: pathFilter().trim(), rev: selectedSha() ?? undefined })}
              title="View blame for this file"
            >
              Blame file
            </button>
            <button
              class="collapse-toggle"
              onClick={() => setHistoricalFile({ path: pathFilter().trim(), rev: selectedSha() ?? "HEAD" })}
              title="View this file at current selected revision (or HEAD)"
            >
              View file
            </button>
          </Show>
        </Show>
        <span class="text-muted">{totalCount()} commits</span>
        <button
          class="collapse-toggle"
          classList={{ "diff-mode-active": compareMode() }}
          onClick={() => {
            const next = !compareMode();
            setCompareMode(next);
            if (!next) setCompareSha(null);
          }}
          title="Select two commits to view their diff"
        >
          {compareMode() ? "Exit compare mode" : "Compare revisions"}
        </button>
        <button
          class="collapse-toggle"
          classList={{ "diff-mode-active": searchOpen() }}
          onClick={() => {
            const next = !searchOpen();
            setSearchOpen(next);
            if (!next) handleClearSearch();
          }}
          title="Search history by message, author, content, path, or date"
        >
          {searchOpen() ? "Close search" : "Search"}
        </button>
      </div>

      <Show when={searchOpen()}>
        <div class="history-search-bar">
          <select
            class="history-search-type"
            value={searchType()}
            onChange={(e) => setSearchType(e.currentTarget.value as any)}
          >
            <option value="message">Message</option>
            <option value="author">Author</option>
            <option value="content_pickaxe">Content (-S)</option>
            <option value="content_regex">Content regex (-G)</option>
            <option value="path">Path</option>
          </select>
          <input
            class="history-search-input"
            type="text"
            placeholder={
              searchType() === "content_pickaxe"
                ? "String added/removed (-S)..."
                : searchType() === "content_regex"
                ? "Regex matching diff (-G)..."
                : searchType() === "author"
                ? "Author name or email..."
                : searchType() === "path"
                ? "Path..."
                : "Commit message..."
            }
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void executeSearch(false);
            }}
          />
          <input
            class="history-search-date"
            type="text"
            placeholder="Since date"
            value={searchSince()}
            onInput={(e) => setSearchSince(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void executeSearch(false);
            }}
          />
          <input
            class="history-search-date"
            type="text"
            placeholder="Until date"
            value={searchUntil()}
            onInput={(e) => setSearchUntil(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void executeSearch(false);
            }}
          />
          <button
            class="collapse-toggle"
            disabled={isSearching()}
            onClick={() => void executeSearch(false)}
          >
            {isSearching() ? "Searching…" : "Search"}
          </button>
          <Show when={isSearching()}>
            <button class="collapse-toggle" onClick={() => void handleCancelSearch()}>
              Cancel
            </button>
          </Show>
          <button class="collapse-toggle" onClick={handleClearSearch}>
            Clear
          </button>
        </div>
      </Show>

      <Show when={searchError()}>
        <p class="diff-apply-error">{searchError()}</p>
      </Show>

      <Show when={error()}>
        <p class="diff-apply-error">{error()}</p>
      </Show>

      <Show
        when={searchResults()}
        fallback={
          <>
            <Show when={props.hasUncommittedChanges}>
              <button class="history-uncommitted-entry" onClick={props.onSelectUncommitted}>
                <span class="history-sha">●</span>
                <span class="history-subject">Uncommitted changes</span>
              </button>
            </Show>
            <div
              class="history-scroll"
              ref={containerEl}
              onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
            >
              <div class="history-spacer" style={{ height: `${totalCount() * ROW_HEIGHT}px` }}>
                <canvas class="history-graph-canvas" ref={canvasEl} style={{ width: `${GRAPH_WIDTH}px` }} />
                <For each={rowsToRender()}>
                  {(row) => (
                    <button
                      class="history-row"
                      classList={{
                        "history-row-selected": row.commit?.sha === selectedSha(),
                        "history-row-compare": row.commit?.sha === compareSha(),
                      }}
                      style={{ transform: `translateY(${row.index * ROW_HEIGHT}px)`, "padding-left": `${GRAPH_WIDTH}px` }}
                      onClick={(e) => row.commit && onRowClick(e, row.commit.sha)}
                    >
                      <Show when={row.commit} fallback={<span class="text-muted">Loading…</span>}>
                        {(commit) => (
                          <>
                            <span class="history-sha">{commit().sha.slice(0, 7)}</span>
                            <span class="history-subject">{commit().subject}</span>
                            <Show when={commit().rename_from}>
                              <span class="badge badge-rename" title={`Renamed from ${commit().rename_from}`}>
                                renamed from {commit().rename_from}
                              </span>
                            </Show>
                            <span class="history-author">{commit().author_name}</span>
                          </>
                        )}
                      </Show>
                    </button>
                  )}
                </For>
              </div>
            </div>
          </>
        }
      >
        {(res) => (
          <div class="history-search-results">
            <div class="history-search-status">
              <span>
                Found {res().commits.length} commits
                {res().truncated ? " (truncated at limit)" : ""}
              </span>
              <Show when={res().truncated}>
                <button
                  class="working-copy-action"
                  disabled={isSearching()}
                  onClick={() => void executeSearch(true)}
                >
                  {isSearching() ? "Loading…" : "Load more"}
                </button>
              </Show>
            </div>
            <Show when={res().commits.length === 0}>
              <p class="text-muted">No commits match the search criteria.</p>
            </Show>
            <div class="history-search-results-list">
              <For each={res().commits}>
                {(commit) => (
                  <button
                    class="history-search-row"
                    classList={{
                      "history-search-row-selected": commit.sha === selectedSha(),
                      "history-row-compare": commit.sha === compareSha(),
                    }}
                    onClick={(e) => onRowClick(e, commit.sha)}
                  >
                    <span class="history-sha">{commit.sha.slice(0, 7)}</span>
                    <Show when={commit.rename_from}>
                      <span class="badge badge-rename" title={`Renamed from ${commit.rename_from}`}>
                        renamed from {commit.rename_from}
                      </span>
                    </Show>
                    <span class="history-subject">{commit.subject}</span>
                    <span class="history-author">{commit.author_name}</span>
                    <span class="history-date">{commit.author_date.slice(0, 10)}</span>
                  </button>
                )}
              </For>
            </div>
          </div>
        )}
      </Show>

      <Show when={historicalFile()}>
        {(hf) => (
          <HistoricalFileView
            root={props.root}
            rev={hf().rev}
            path={hf().path}
            onClose={() => setHistoricalFile(null)}
          />
        )}
      </Show>

      <Show when={blameFile()}>
        {(bf) => (
          <div class="history-blame-container">
            <div class="history-blame-header">
              <span>Blame: <strong>{bf().path}</strong></span>
              <button class="collapse-toggle" onClick={() => setBlameFile(null)}>Close blame</button>
            </div>
            <BlameView root={props.root} path={bf().path} initialRev={bf().rev} />
          </div>
        )}
      </Show>

      <Show when={!historicalFile() && !blameFile() && selectedSha() && compareSha()}>
        <RevisionDiffView
          root={props.root}
          oldRev={selectedSha()!}
          oldPath={cache.get(shaIndex.get(selectedSha()!) ?? -1)?.path_at_commit ?? (scope().kind === "Path" ? pathFilter() : undefined)}
          newRev={compareSha()!}
          newPath={cache.get(shaIndex.get(compareSha()!) ?? -1)?.path_at_commit ?? (scope().kind === "Path" ? pathFilter() : undefined)}
          onSwap={() => {
            const s1 = selectedSha();
            const s2 = compareSha();
            setSelectedSha(s2);
            setCompareSha(s1);
          }}
          onClose={() => setCompareSha(null)}
        />
      </Show>

      <Show when={!historicalFile() && !blameFile() && !compareSha() && selectedSha() ? selectedSha() : null}>
        {(sha) => (
          <CommitDetailView
            root={props.root}
            sha={sha()}
            onSelectFileHistory={(p) => {
              setScope({ kind: "Path", path: p });
              setPathFilter(p);
              setSelectedSha(null);
              setCompareSha(null);
            }}
            onOpenBlame={(p, rev) => setBlameFile({ path: p, rev })}
            onViewRevision={(p, rev) => setHistoricalFile({ path: p, rev })}
            onStartRebase={(baseSha, subject) => setRebaseTarget({ baseSha, subject })}
            onStartCherryPick={(sha, subject) => setCherryPickTarget({ sha, subject })}
            onStartRevert={(sha, subject) => setRevertTarget({ sha, subject })}
          />
        )}
      </Show>

      <Show when={integrationNotice()}>
        {(msg) => (
          <div class="integration-notice" style={{ padding: "var(--space-2) var(--space-4)", "background-color": "var(--color-bg-subtle)", "border-bottom": "1px solid var(--color-border-subtle)", display: "flex", "align-items": "center", "justify-content": "space-between" }}>
            <span>{msg()}</span>
            <button class="collapse-toggle" onClick={() => setIntegrationNotice(null)}>Dismiss</button>
          </div>
        )}
      </Show>

      <Show when={rebaseTarget()}>
        {(target) => (
          <InteractiveRebaseModal
            root={props.root}
            baseRef={target().baseSha}
            baseDescription={target().subject}
            onClose={() => setRebaseTarget(null)}
            onSuccess={() => {
              setRebaseTarget(null);
              setIntegrationNotice("Interactive rebase completed successfully.");
              setVersion((v) => v + 1);
            }}
            onPaused={(_conflicts, msg) => {
              setRebaseTarget(null);
              setIntegrationNotice(`Rebase paused: ${msg}`);
              setVersion((v) => v + 1);
            }}
            onNavigateWorkingCopy={props.onSelectUncommitted}
          />
        )}
      </Show>

      <Show when={cherryPickTarget()}>
        {(target) => (
          <CherryPickModal
            root={props.root}
            defaultCommit={target().sha}
            defaultSubject={target().subject}
            onClose={() => setCherryPickTarget(null)}
            onSuccess={(_newHead) => {
              setCherryPickTarget(null);
              setIntegrationNotice("Cherry-pick applied successfully.");
              setVersion((v) => v + 1);
            }}
            onConflict={(_conflicts, msg) => {
              setCherryPickTarget(null);
              setIntegrationNotice(`Cherry-pick paused: ${msg}`);
              setVersion((v) => v + 1);
            }}
            onNavigateWorkingCopy={props.onSelectUncommitted}
          />
        )}
      </Show>

      <Show when={revertTarget()}>
        {(target) => (
          <RevertModal
            root={props.root}
            defaultCommit={target().sha}
            defaultSubject={target().subject}
            onClose={() => setRevertTarget(null)}
            onSuccess={(_newHead) => {
              setRevertTarget(null);
              setIntegrationNotice("Revert completed successfully.");
              setVersion((v) => v + 1);
            }}
            onConflict={(_conflicts, msg) => {
              setRevertTarget(null);
              setIntegrationNotice(`Revert paused: ${msg}`);
              setVersion((v) => v + 1);
            }}
            onNavigateWorkingCopy={props.onSelectUncommitted}
          />
        )}
      </Show>
    </div>
  );
};
