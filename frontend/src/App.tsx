import { type Component, For, Show, createEffect, createResource, createSignal, onCleanup, onMount } from "solid-js";
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
import type { Bookmark, RepositoryState, SubmoduleState } from "./api/types";
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
import { GroupSwitcher } from "./workspace/GroupSwitcher";
import { TabStrip } from "./workspace/TabStrip";
import { useWorkspace } from "./workspace/use-workspace";
import "./layout/layout.css";
import "./features/repository/repository.css";
import "./features/submodules/submodule-matrix.css";
import "./features/operation-log/operation-log.css";
import "./workspace/workspace.css";

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
  const [openError, setOpenError] = createSignal<string | null>(null);
  const [watchDegraded, setWatchDegraded] = createSignal<Record<string, string>>({});
  let pathInputEl: HTMLInputElement | undefined;
  function setPathInputEl(el: HTMLInputElement) {
    pathInputEl = el;
  }

  const workspace = useWorkspace();
  const activeViewPath = () => workspace.activeTab()?.path ?? null;

  // Per-path caches, invalidated by the watcher on a real change.
  const repoCache = new Map<string, RepositoryState>();
  const submoduleCache = new Map<string, SubmoduleState[]>();

  const [bookmarks, { refetch: refetchBookmarks }] = createResource(getBookmarks);
  const [settingsResult] = createResource(getSettings);
  const [operationLog, { refetch: refetchOperationLog }] = createResource(() => workspace.state().groups.length > 0, () =>
    getOperationLog(),
  );

  const [repoState, { refetch: refetchRepoState }] = createResource(activeViewPath, async (path) => {
    const cached = repoCache.get(path);
    if (cached) return cached;
    const state = await getRepositoryState(path);
    repoCache.set(path, state);
    return state;
  });

  const [submodules, { refetch: refetchSubmodules }] = createResource(activeViewPath, async (path) => {
    const cached = submoduleCache.get(path);
    if (cached) return cached;
    const list = await getSubmoduleMatrix(path);
    submoduleCache.set(path, list);
    return list;
  });

  function invalidate(path: string) {
    repoCache.delete(path);
    submoduleCache.delete(path);
    if (path === activeViewPath()) {
      refetchRepoState();
      refetchSubmodules();
    }
  }

  // Focus the primary action whenever no group is open.
  createEffect(() => {
    if (workspace.state().groups.length === 0) pathInputEl?.focus();
  });

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
    invalidate(root);
    refetchOperationLog();
  }).then((un) => (unlistenChanged = un));
  onWatchDegraded((info) => {
    setWatchDegraded((prev) => ({ ...prev, [info.root]: info.reason }));
  }).then((un) => (unlistenDegraded = un));
  onCleanup(() => {
    unlistenChanged?.();
    unlistenDegraded?.();
  });

  onMount(() => {
    function onKeyDown(e: KeyboardEvent) {
      const groupId = workspace.activeGroup()?.id;
      if (!groupId) return;
      if (e.ctrlKey && e.key === "Tab") {
        e.preventDefault();
        workspace.cycleTab(groupId, e.shiftKey ? -1 : 1);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    onCleanup(() => window.removeEventListener("keydown", onKeyDown));

    // Re-arm a watcher per restored group.
    for (const group of workspace.state().groups) {
      void startWatching(group.id);
    }
  });

  async function openPath(path: string) {
    if (!path.trim()) return;
    setOpenError(null);
    try {
      const root = await resolveRepositoryRoot(path);
      const isNewGroup = !workspace.state().groups.some((g) => g.id === root);
      workspace.openRepository(root);
      if (isNewGroup) {
        void startWatching(root);
      }
    } catch (e) {
      setOpenError(String(e));
    }
  }

  function closeGroup(groupId: string) {
    workspace.closeGroup(groupId);
    void stopWatching(groupId);
    setWatchDegraded((prev) => {
      const next = { ...prev };
      delete next[groupId];
      return next;
    });
  }

  function drillIntoSubmodule(path: string, name: string) {
    const groupId = workspace.activeGroup()?.id;
    if (!groupId) return;
    workspace.openSubmoduleTab(groupId, path, name);
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
    const path = activeViewPath();
    if (path) invalidate(path);
    refetchOperationLog();
  }

  return (
    <div class="app-shell">
      <div class="app-titlebar">
        <span>GitTree</span>
        <Show when={workspace.activeGroup()}>
          <button class="collapse-toggle" onClick={refreshAll}>
            Refresh
          </button>
          <button class="collapse-toggle" onClick={() => void toggleBookmark(workspace.activeGroup()!.rootPath)}>
            {bookmarks()?.some((b) => b.root === workspace.activeGroup()!.rootPath) ? "Remove bookmark" : "Add bookmark"}
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

      <Show when={workspace.state().groups.length > 0}>
        <GroupSwitcher
          groups={workspace.state().groups}
          activeGroupId={workspace.state().activeGroupId}
          onSelect={(id) => workspace.setActiveGroup(id)}
          onClose={closeGroup}
        />
      </Show>
      <Show when={workspace.activeGroup()}>
        {(group) => (
          <TabStrip
            tabs={group().tabs}
            activeTabId={group().activeTabId}
            onSelect={(tabId) => workspace.setActiveTab(group().id, tabId)}
            onClose={(tabId) => workspace.closeTab(group().id, tabId)}
          />
        )}
      </Show>

      <Show when={workspace.activeGroup() && watchDegraded()[workspace.activeGroup()!.id]}>
        <div class="watch-degraded-banner">Filesystem watching degraded: {watchDegraded()[workspace.activeGroup()!.id]}</div>
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
                ref={setPathInputEl}
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

          <Show when={workspace.activeGroup()} fallback={<p class="text-muted">Open a repository to begin.</p>}>
            <Show when={repoState()}>{(state) => <RepositoryStatus state={state()} />}</Show>
            <Show when={submodules() && submodules()!.length > 0}>
              <SubmoduleMatrix submodules={submodules()!} onDrillIn={drillIntoSubmodule} />
            </Show>
            <Show when={submodules.loading}>
              <p class="text-muted">Loading submodules…</p>
            </Show>
          </Show>
        </main>

        <Show when={!detailCollapsed() && workspace.activeGroup()}>
          <Resizer label="Resize detail panel" onResize={(d) => setDetailWidth(clamp(detailWidth() - d, 240, 640))} />
          <aside class="pane-detail" style={{ width: `${clamp(detailWidth(), 240, 640)}px` }} aria-label="Detail panel">
            <OperationLogView entries={operationLog() ?? []} />
          </aside>
        </Show>
      </div>
    </div>
  );
};
