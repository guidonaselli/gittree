import { type Component, For, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import {
  getHistoryCount,
  getHistoryGraph,
  getHistoryPage,
  type CommitSummary,
  type GraphRow,
  type HistoryScope,
} from "../../api/commands";

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
}> = (props) => {
  const [scope, setScope] = createSignal<HistoryScope>({ kind: "CurrentBranch" });
  const [pathFilter, setPathFilter] = createSignal("");
  const [totalCount, setTotalCount] = createSignal(0);
  const [scrollTop, setScrollTop] = createSignal(0);
  const [viewportHeight, setViewportHeight] = createSignal(400);
  const [version, setVersion] = createSignal(0);
  const [graphVersion, setGraphVersion] = createSignal(0);
  const [error, setError] = createSignal<string | null>(null);

  let cache = new Map<number, CommitSummary>();
  let loadedPages = new Set<number>();
  let loadingPages = new Set<number>();
  let graphRows: GraphRow[] = [];
  let shaIndex = new Map<string, number>();
  let containerEl: HTMLDivElement | undefined;
  let canvasEl: HTMLCanvasElement | undefined;

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

  return (
    <div class="history-view">
      <div class="history-scope-bar">
        <select
          class="history-scope-select"
          value={scope().kind}
          onChange={(e) => setScope(e.currentTarget.value === "Path" ? { kind: "Path", path: pathFilter() } : ({ kind: e.currentTarget.value } as HistoryScope))}
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
            placeholder="path/to/file"
            value={pathFilter()}
            onInput={(e) => setPathFilter(e.currentTarget.value)}
          />
        </Show>
        <span class="text-muted">{totalCount()} commits</span>
      </div>
      <Show when={error()}>
        <p class="diff-apply-error">{error()}</p>
      </Show>
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
              <div
                class="history-row"
                style={{ transform: `translateY(${row.index * ROW_HEIGHT}px)`, "padding-left": `${GRAPH_WIDTH}px` }}
              >
                <Show when={row.commit} fallback={<span class="text-muted">Loading…</span>}>
                  {(commit) => (
                    <>
                      <span class="history-sha">{commit().sha.slice(0, 7)}</span>
                      <span class="history-subject">{commit().subject}</span>
                      <span class="history-author">{commit().author_name}</span>
                    </>
                  )}
                </Show>
              </div>
            )}
          </For>
        </div>
      </div>
    </div>
  );
};
