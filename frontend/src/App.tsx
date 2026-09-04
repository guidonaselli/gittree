import { type Component, For, Show, createEffect, createResource, createSignal, onCleanup, onMount } from "solid-js";
import {
  addBookmark,
  amendWorkingCopy,
  type CommitOptions,
  commitWorkingCopy,
  deleteUntrackedWorkingCopyPaths,
  getCommitMessageTemplate,
  discardWorkingCopyPaths,
  getBookmarks,
  getDesktopPalette,
  getOperationLog,
  getRepositoryState,
  getSettings,
  getSubmoduleMatrix,
  getWorkingCopyStatus,
  initRepository,
  isHeadPublished,
  removeBookmark,
  resolveRepositoryRoot,
  saveSettings,
  stageWorkingCopyPaths,
  stashWorkingCopyPaths,
  startWatching,
  stopWatching,
  unstageWorkingCopyPaths,
} from "./api/commands";
import { onDesktopThemeChanged, onRepositoryChanged, onWatchDegraded } from "./api/events";
import { isKnown, type Bookmark, type RepositoryState, type Resolved, type SubmoduleState, type WorkingCopyStatus } from "./api/types";
import { WorkingCopyView } from "./features/working-copy/WorkingCopyView";
import { HistoryView } from "./features/history/HistoryView";
import { PALETTE_TOKENS, resolveTheme } from "./theme/apply-palette";
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
import "./features/working-copy/working-copy.css";
import "./features/history/history.css";
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
  const [mainView, setMainView] = createSignal<"working-copy" | "history">("working-copy");
  const [stageError, setStageError] = createSignal<string | null>(null);
  const [commitError, setCommitError] = createSignal<string | null>(null);
  const [hookOutput, setHookOutput] = createSignal<string | null>(null);
  const [offerInitAt, setOfferInitAt] = createSignal<string | null>(null);
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
  const workingCopyCache = new Map<string, Resolved<WorkingCopyStatus>>();

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

  const [workingCopy, { refetch: refetchWorkingCopy }] = createResource(activeViewPath, async (path) => {
    const cached = workingCopyCache.get(path);
    if (cached) return cached;
    const status = await getWorkingCopyStatus(path);
    workingCopyCache.set(path, status);
    return status;
  });

  function hasUncommittedChanges(): boolean {
    const status = workingCopy();
    if (!status || !isKnown(status)) return false;
    const value = status.value;
    return value.changed.length > 0 || value.untracked.length > 0 || value.conflicted.length > 0;
  }

  const [commitMessageTemplate] = createResource(activeViewPath, getCommitMessageTemplate);

  function invalidate(path: string) {
    repoCache.delete(path);
    workingCopyCache.delete(path);
    submoduleCache.delete(path);
    if (path === activeViewPath()) {
      refetchRepoState();
      refetchSubmodules();
      refetchWorkingCopy();
    }
  }

  async function stagePaths(paths: string[]) {
    const root = activeViewPath();
    if (!root || paths.length === 0) return;
    try {
      await stageWorkingCopyPaths(root, paths);
      setStageError(null);
    } catch (err) {
      setStageError(String(err));
    } finally {
      invalidate(root);
    }
  }

  async function unstagePaths(paths: string[]) {
    const root = activeViewPath();
    if (!root || paths.length === 0) return;
    try {
      await unstageWorkingCopyPaths(root, paths);
      setStageError(null);
    } catch (err) {
      setStageError(String(err));
    } finally {
      invalidate(root);
    }
  }

  async function discardPaths(paths: string[]) {
    const root = activeViewPath();
    if (!root || paths.length === 0) return;
    try {
      await discardWorkingCopyPaths(root, paths);
      setStageError(null);
    } catch (err) {
      setStageError(String(err));
    } finally {
      invalidate(root);
    }
  }

  async function deleteUntrackedPaths(paths: string[]) {
    const root = activeViewPath();
    if (!root || paths.length === 0) return;
    try {
      await deleteUntrackedWorkingCopyPaths(root, paths);
      setStageError(null);
    } catch (err) {
      setStageError(String(err));
    } finally {
      invalidate(root);
    }
  }

  async function stashPaths(paths: string[]) {
    const root = activeViewPath();
    if (!root || paths.length === 0) return;
    try {
      await stashWorkingCopyPaths(root, paths, "GitTree discard");
      setStageError(null);
    } catch (err) {
      setStageError(String(err));
    } finally {
      invalidate(root);
    }
  }

  async function commitStaged(message: string, options: CommitOptions) {
    const root = activeViewPath();
    if (!root) return;
    try {
      const output = await commitWorkingCopy(root, message, options);
      setCommitError(null);
      setHookOutput(output || null);
    } catch (err) {
      setCommitError(String(err));
      setHookOutput(null);
    } finally {
      invalidate(root);
    }
  }

  async function checkHeadPublished(): Promise<boolean> {
    const root = activeViewPath();
    if (!root) return false;
    return isHeadPublished(root);
  }

  async function amendHead(message: string, options: CommitOptions) {
    const root = activeViewPath();
    if (!root) return;
    try {
      const output = await amendWorkingCopy(root, message, options);
      setCommitError(null);
      setHookOutput(output || null);
    } catch (err) {
      setCommitError(String(err));
      setHookOutput(null);
    } finally {
      invalidate(root);
    }
  }

  // Focus the primary action whenever no group is open.
  createEffect(() => {
    if (workspace.state().groups.length === 0) pathInputEl?.focus();
  });

  const [desktopPalette, { refetch: refetchDesktopPalette }] = createResource(getDesktopPalette);

  function applyTheme(explicit: string | null | undefined) {
    const root = document.documentElement;
    const { dataTheme, tokens } = resolveTheme(explicit, desktopPalette() ?? undefined);
    for (const token of PALETTE_TOKENS) root.style.removeProperty(token);
    if (dataTheme) root.dataset.theme = dataTheme;
    else delete root.dataset.theme;
    for (const [token, value] of Object.entries(tokens)) root.style.setProperty(token, value);
  }

  // Explicit theme override wins over desktop-sync, which wins over the
  // system light/dark default.
  createEffect(() => {
    applyTheme(settingsResult()?.settings.theme);
  });

  onDesktopThemeChanged(() => refetchDesktopPalette());

  async function setTheme(theme: string) {
    const current = settingsResult()?.settings ?? { concurrency: 8, theme: null };
    const next = { ...current, theme: theme === "system" ? null : theme };
    await saveSettings(next);
    applyTheme(next.theme);
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

  function activateRoot(root: string) {
    const isNewGroup = !workspace.state().groups.some((g) => g.id === root);
    workspace.openRepository(root);
    if (isNewGroup) {
      void startWatching(root);
    }
  }

  async function openPath(path: string) {
    if (!path.trim()) return;
    setOpenError(null);
    setOfferInitAt(null);
    try {
      const result = await resolveRepositoryRoot(path);
      if (result.kind === "NotARepository") {
        setOfferInitAt(path);
        return;
      }
      activateRoot(result.root);
    } catch (e) {
      setOpenError(String(e));
    }
  }

  async function confirmInit(path: string) {
    setOpenError(null);
    try {
      const root = await initRepository(path);
      setOfferInitAt(null);
      activateRoot(root);
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
              <Show when={offerInitAt()}>
                {(path) => (
                  <div class="init-offer">
                    <p class="text-muted">No repository found at {path()}.</p>
                    <button onClick={() => void confirmInit(path())}>Initialize a repository here</button>
                    <button class="collapse-toggle" onClick={() => setOfferInitAt(null)}>
                      Cancel
                    </button>
                  </div>
                )}
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
            <div class="main-view-switcher">
              <button
                class="collapse-toggle"
                aria-pressed={mainView() === "working-copy"}
                onClick={() => setMainView("working-copy")}
              >
                Working copy
              </button>
              <button
                class="collapse-toggle"
                aria-pressed={mainView() === "history"}
                onClick={() => setMainView("history")}
              >
                History
              </button>
            </div>
            <Show when={mainView() === "history"}>
              <HistoryView
                root={activeViewPath()!}
                hasUncommittedChanges={hasUncommittedChanges()}
                onSelectUncommitted={() => setMainView("working-copy")}
              />
            </Show>
            <Show when={mainView() === "working-copy"}>
              <Show when={stageError()}>
                <p class="diff-apply-error">{stageError()}</p>
              </Show>
              <Show when={workingCopy()}>
                {(status) => (
                  <WorkingCopyView
                    root={activeViewPath()!}
                    status={status()}
                    onStage={stagePaths}
                    onUnstage={unstagePaths}
                    onDiscard={discardPaths}
                    onDeleteUntracked={deleteUntrackedPaths}
                    onStash={stashPaths}
                    onHunksChanged={() => invalidate(activeViewPath()!)}
                    onCommit={commitStaged}
                    commitError={commitError()}
                    hookOutput={hookOutput()}
                    messageTemplate={commitMessageTemplate() ?? null}
                    onCheckHeadPublished={checkHeadPublished}
                    onAmend={amendHead}
                  />
                )}
              </Show>
            </Show>
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
