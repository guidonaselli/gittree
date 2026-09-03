import { type Component, For, Show, createEffect, createResource, createSignal, onCleanup } from "solid-js";
import {
  addBookmark,
  getBookmarks,
  getOperationLog,
  getRepositoryState,
  getSettings,
  getSubmoduleMatrix,
  removeBookmark,
  resolveRepositoryRoot,
  saveSettings,
  startWatching,
  stopWatching,
} from "./api/commands";
import { onRepositoryChanged, onWatchDegraded } from "./api/events";
import type { Bookmark } from "./api/types";
import { OperationLogView } from "./features/operation-log/OperationLogView";
import { RepositoryStatus } from "./features/repository/RepositoryStatus";
import { SubmoduleMatrix } from "./features/submodules/SubmoduleMatrix";
import {
  useDetailPanelCollapsed,
  useDetailPanelWidth,
  useSidebarCollapsed,
  useSidebarWidth,
} from "./layout/persisted-layout";
import { Resizer } from "./layout/Resizer";
import "./layout/layout.css";
import "./features/repository/repository.css";
import "./features/submodules/submodule-matrix.css";
import "./features/operation-log/operation-log.css";

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function groupBookmarks(bookmarks: Bookmark[]): Map<string, Bookmark[]> {
  const groups = new Map<string, Bookmark[]>();
  const sorted = [...bookmarks].sort((a, b) => a.order - b.order);
  for (const b of sorted) {
    const key = b.group ?? "";
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(b);
  }
  return groups;
}

export const App: Component = () => {
  const [sidebarWidth, setSidebarWidth] = useSidebarWidth();
  const [sidebarCollapsed, setSidebarCollapsed] = useSidebarCollapsed();
  const [detailWidth, setDetailWidth] = useDetailPanelWidth();
  const [detailCollapsed, setDetailCollapsed] = useDetailPanelCollapsed();

  const [pathInput, setPathInput] = createSignal("");
  const [activeRoot, setActiveRoot] = createSignal<string | null>(null);
  const [openError, setOpenError] = createSignal<string | null>(null);
  const [watchDegradedReason, setWatchDegradedReason] = createSignal<string | null>(null);

  const [bookmarks, { refetch: refetchBookmarks }] = createResource(getBookmarks);
  const [settingsResult] = createResource(getSettings);
  const [repoState, { refetch: refetchRepoState }] = createResource(activeRoot, (root) => getRepositoryState(root));
  const [submodules, { refetch: refetchSubmodules }] = createResource(activeRoot, (root) => getSubmoduleMatrix(root));
  const [operationLog, { refetch: refetchOperationLog }] = createResource(activeRoot, () => getOperationLog());

  // Explicit theme override wins over the system preference default.
  createEffect(() => {
    const theme = settingsResult()?.settings.theme;
    if (theme === "light" || theme === "dark") {
      document.documentElement.dataset.theme = theme;
    } else {
      delete document.documentElement.dataset.theme;
    }
  });

  async function setTheme(theme: string) {
    const current = settingsResult()?.settings ?? { concurrency: 8, theme: null };
    const next = { ...current, theme: theme === "system" ? null : theme };
    await saveSettings(next);
    if (next.theme === "light" || next.theme === "dark") {
      document.documentElement.dataset.theme = next.theme;
    } else {
      delete document.documentElement.dataset.theme;
    }
  }

  let unlistenChanged: (() => void) | undefined;
  let unlistenDegraded: (() => void) | undefined;
  onRepositoryChanged((root) => {
    if (root === activeRoot()) {
      refetchRepoState();
      refetchSubmodules();
      refetchOperationLog();
    }
  }).then((un) => (unlistenChanged = un));
  onWatchDegraded((info) => {
    if (info.root === activeRoot()) setWatchDegradedReason(info.reason);
  }).then((un) => (unlistenDegraded = un));
  onCleanup(() => {
    unlistenChanged?.();
    unlistenDegraded?.();
  });

  async function openPath(path: string) {
    if (!path.trim()) return;
    setOpenError(null);
    setWatchDegradedReason(null);
    try {
      const root = await resolveRepositoryRoot(path);
      const previous = activeRoot();
      if (previous && previous !== root) void stopWatching(previous);
      setActiveRoot(root);
      void startWatching(root);
    } catch (e) {
      setOpenError(String(e));
    }
  }

  async function toggleBookmark(root: string) {
    const isBookmarked = bookmarks()?.some((b) => b.root === root);
    if (isBookmarked) {
      await removeBookmark(root);
    } else {
      await addBookmark(root, null);
    }
    refetchBookmarks();
  }

  function refreshAll() {
    refetchRepoState();
    refetchSubmodules();
    refetchOperationLog();
  }

  return (
    <div class="app-shell">
      <div class="app-titlebar">
        <span>GitTree</span>
        <Show when={activeRoot()}>
          <span class="text-muted">— {activeRoot()}</span>
          <button class="collapse-toggle" onClick={refreshAll}>
            Refresh
          </button>
          <button class="collapse-toggle" onClick={() => void toggleBookmark(activeRoot()!)}>
            {bookmarks()?.some((b) => b.root === activeRoot()) ? "Remove bookmark" : "Add bookmark"}
          </button>
          <button
            class="collapse-toggle"
            aria-expanded={!detailCollapsed()}
            onClick={() => setDetailCollapsed(!detailCollapsed())}
          >
            {detailCollapsed() ? "Show detail panel" : "Hide detail panel"}
          </button>
        </Show>
        <label class="theme-select">
          Theme:
          <select
            value={settingsResult()?.settings.theme ?? "system"}
            onChange={(e) => void setTheme(e.currentTarget.value)}
          >
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
      </div>

      <Show when={watchDegradedReason()}>
        <div class="watch-degraded-banner">Filesystem watching degraded: {watchDegradedReason()}</div>
      </Show>

      <div class="app-body">
        <Show when={!sidebarCollapsed()}>
          <nav
            class="pane-sidebar"
            style={{ width: `${clamp(sidebarWidth(), 180, 480)}px` }}
            aria-label="Repository bookmarks"
          >
            <div class="repo-status" style={{ "flex-direction": "column", "align-items": "stretch" }}>
              <label for="open-path-input">Open repository</label>
              <input
                id="open-path-input"
                type="text"
                placeholder="/path/to/repository"
                value={pathInput()}
                onInput={(e) => setPathInput(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void openPath(pathInput());
                }}
              />
              <button onClick={() => void openPath(pathInput())}>Open</button>
              <Show when={openError()}>
                <div class="text-danger">{openError()}</div>
              </Show>
            </div>
            <For each={[...groupBookmarks(bookmarks() ?? []).entries()]}>
              {([group, items]) => (
                <div class="bookmark-group">
                  <Show when={group}>
                    <div class="bookmark-group-label">{group}</div>
                  </Show>
                  <ul>
                    <For each={items}>
                      {(b) => (
                        <li>
                          <button class="collapse-toggle bookmark-item" onClick={() => void openPath(b.root)}>
                            {b.root}
                          </button>
                        </li>
                      )}
                    </For>
                  </ul>
                </div>
              )}
            </For>
          </nav>
          <Resizer label="Resize sidebar" onResize={(d) => setSidebarWidth(clamp(sidebarWidth() + d, 180, 480))} />
        </Show>

        <main class="pane-content">
          <button
            class="collapse-toggle"
            aria-expanded={!sidebarCollapsed()}
            onClick={() => setSidebarCollapsed(!sidebarCollapsed())}
          >
            {sidebarCollapsed() ? "Show sidebar" : "Hide sidebar"}
          </button>

          <Show when={activeRoot()} fallback={<p class="text-muted">Open a repository to begin.</p>}>
            <Show when={repoState()}>{(state) => <RepositoryStatus state={state()} />}</Show>
            <Show when={submodules() && submodules()!.length > 0}>
              <SubmoduleMatrix submodules={submodules()!} />
            </Show>
            <Show when={submodules.loading}>
              <p class="text-muted">Loading submodules…</p>
            </Show>
          </Show>
        </main>

        <Show when={!detailCollapsed() && activeRoot()}>
          <Resizer label="Resize detail panel" onResize={(d) => setDetailWidth(clamp(detailWidth() - d, 240, 640))} />
          <aside class="pane-detail" style={{ width: `${clamp(detailWidth(), 240, 640)}px` }} aria-label="Detail panel">
            <OperationLogView entries={operationLog() ?? []} />
          </aside>
        </Show>
      </div>
    </div>
  );
};
