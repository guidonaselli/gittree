import { type Component, For, Show, createEffect, createMemo, createResource, createSignal, onCleanup, onMount } from "solid-js";
import {
  addBookmark,
  amendWorkingCopy,
  type CommitOptions,
  commitWorkingCopy,
  deleteUntrackedWorkingCopyPaths,
  getActiveOperation,
  getCommitMessageTemplate,
  discardWorkingCopyPaths,
  cancelWorkingCopyStatus,
  getCliRepoArg,
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
  pickFolder,
  resolveRepositoryRoot,
  saveSettings,
  stageWorkingCopyPaths,
  stashWorkingCopyPaths,
  startWatching,
  stopWatching,
  unstageWorkingCopyPaths,
  cancelSyncNetworkOperation,
  submitAskpassResponse,
  cancelAskpassResponse,
  getUserThemes,
  type HistoryScope,
} from "./api/commands";
import { onDesktopThemeChanged, onUserThemesChanged, onRepositoryChanged, onWatchDegraded, onAskpassPrompt } from "./api/events";
import { branchLabel, isKnown, type AskpassPromptPayload, type RepositoryState, type Resolved, type Settings, type SubmoduleMatrixResult, type WorkingCopyStatus } from "./api/types";
import { WorkingCopyView } from "./features/working-copy/WorkingCopyView";
import { HistoryView } from "./features/history/HistoryView";
import { BranchesView } from "./features/branches/BranchesView";
import { TagsView } from "./features/tags/TagsView";
import { StashesView } from "./features/stashes/StashesView";
import { ReflogView } from "./features/reflog/ReflogView";
import { FetchModal } from "./features/sync/FetchModal";
import { PullModal } from "./features/sync/PullModal";
import { PushModal } from "./features/sync/PushModal";
import { AskpassModal } from "./features/sync/AskpassModal";
import { PALETTE_TOKENS, resolveTheme } from "./theme/apply-palette";
import { OperationLogView } from "./features/operation-log/OperationLogView";
import { RepositorySidebar, type NavView } from "./features/repository/RepositorySidebar";
import { SubmoduleMatrix } from "./features/submodules/SubmoduleMatrix";
import { SyncToolbar } from "./features/sync/SyncToolbar";
import { minimizeWindow, toggleMaximizeWindow, closeWindow } from "./window";
import { ActiveOperationBanner } from "./features/integration/ActiveOperationBanner";
import { ConflictMarkerGuardModal } from "./features/conflicts/ConflictMarkerGuardModal";
import { CommandPalette, type CommandPaletteItem } from "./features/command-palette/CommandPalette";
import { KeybindingsModal } from "./keybindings/KeybindingsModal";
import { CommandErrorModal } from "./features/command-error/CommandErrorModal";
import { SettingsModal } from "./features/settings/SettingsModal";
import { Icon } from "./ui/Icon";
import {
  getEffectiveShortcut,
  loadCustomBindings,
  matchesEvent,
  saveCustomBindings,
} from "./keybindings/keybindings";
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
import "./features/integration/integration.css";
import "./workspace/workspace.css";

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

export const App: Component = () => {
  const [sidebarWidth, setSidebarWidth] = useSidebarWidth();
  const [sidebarCollapsed, setSidebarCollapsed] = useSidebarCollapsed();
  const [detailWidth, setDetailWidth] = useDetailPanelWidth();
  const [detailCollapsed, setDetailCollapsed] = useDetailPanelCollapsed();

  const [pathInput, setPathInput] = createSignal("");
  const [openError, setOpenError] = createSignal<string | null>(null);
  const [mainView, setMainView] = createSignal<NavView>("working-copy");
  const [reflogTargetRef, setReflogTargetRef] = createSignal<string>("HEAD");
  const [historyScope, setHistoryScope] = createSignal<HistoryScope | undefined>(undefined);
  const [stageError, setStageError] = createSignal<string | null>(null);
  const [commitError, setCommitError] = createSignal<string | null>(null);
  const [hookOutput, setHookOutput] = createSignal<string | null>(null);
  const [offerInitAt, setOfferInitAt] = createSignal<string | null>(null);
  const [watchDegraded, setWatchDegraded] = createSignal<Record<string, string>>({});
  const [showFetchModal, setShowFetchModal] = createSignal(false);
  const [showPullModal, setShowPullModal] = createSignal(false);
  const [showPushModal, setShowPushModal] = createSignal(false);
  const [showCommandPalette, setShowCommandPalette] = createSignal(false);
  const [showKeybindingsModal, setShowKeybindingsModal] = createSignal(false);
  const [showSettingsModal, setShowSettingsModal] = createSignal(false);
  const [customKeybindings, setCustomKeybindings] = createSignal<Record<string, string>>(loadCustomBindings());
  const [activeCommandError, setActiveCommandError] = createSignal<{
    title?: string;
    command?: string;
    exitStatus?: number;
    stderr: string;
  } | null>(null);

  function handleSaveBindings(bindings: Record<string, string>) {
    setCustomKeybindings(bindings);
    saveCustomBindings(bindings);
  }

  const [isSyncing, setIsSyncing] = createSignal(false);
  const [askpassPrompt, setAskpassPrompt] = createSignal<AskpassPromptPayload | null>(null);
  let pathInputEl: HTMLInputElement | undefined;
  function setPathInputEl(el: HTMLInputElement) {
    pathInputEl = el;
  }

  const workspace = useWorkspace();
  const activeViewPath = () => workspace.activeTab()?.path ?? null;

  // Per-path caches, invalidated by the watcher on a real change.
  const repoCache = new Map<string, RepositoryState>();
  const submoduleCache = new Map<string, SubmoduleMatrixResult>();
  const workingCopyCache = new Map<string, Resolved<WorkingCopyStatus>>();

  const [bookmarks, { refetch: refetchBookmarks }] = createResource(getBookmarks);
  const [settingsResult, { refetch: refetchSettings }] = createResource(getSettings);
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

  const [integrationNotice, setIntegrationNotice] = createSignal<string | null>(null);
  const [activeOperation, { refetch: refetchActiveOperation }] = createResource(
    activeViewPath,
    async (path) => {
      if (!path) return null;
      try {
        return await getActiveOperation(path);
      } catch {
        return null;
      }
    }
  );

  const aheadBehind = createMemo(() => {
    const s = repoState();
    return s && isKnown(s.ahead_behind) ? s.ahead_behind.value : null;
  });

  function hasUncommittedChanges(): boolean {
    const status = workingCopy();
    if (!status || !isKnown(status)) return false;
    const value = status.value;
    return value.changed.length > 0 || value.untracked.length > 0 || value.conflicted.length > 0;
  }

  const [commitMessageTemplate] = createResource(activeViewPath, getCommitMessageTemplate);

  function invalidate(path: string, options: { submodules?: boolean } = {}) {
    repoCache.delete(path);
    workingCopyCache.delete(path);
    if (options.submodules) {
      submoduleCache.delete(path);
    }
    if (path === activeViewPath()) {
      refetchRepoState();
      if (options.submodules) {
        refetchSubmodules();
      }
      refetchWorkingCopy();
      refetchActiveOperation();
    }
  }

  const [conflictMarkerRefusal, setConflictMarkerRefusal] = createSignal<{
    paths: string[];
    file: string;
    marker_lines: number[];
    preview_lines: string[];
  } | null>(null);

  async function stagePaths(paths: string[], overrideMarkers = false) {
    const root = activeViewPath();
    if (!root || paths.length === 0) return;
    try {
      const outcome = await stageWorkingCopyPaths(root, paths, overrideMarkers);
      if (outcome && outcome.status === "marker_refusal") {
        setConflictMarkerRefusal({
          paths,
          file: outcome.file,
          marker_lines: outcome.marker_lines,
          preview_lines: outcome.preview_lines,
        });
        return;
      }
      setStageError(null);
    } catch (err) {
      setStageError(String(err));
    } finally {
      invalidate(root);
      refetchActiveOperation();
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
  const [userThemesResult, { refetch: refetchUserThemes }] = createResource(getUserThemes);

  function applyTheme(explicit: string | null | undefined) {
    const root = document.documentElement;
    const { dataTheme, tokens } = resolveTheme(
      explicit,
      desktopPalette() ?? undefined,
      userThemesResult()?.themes,
    );
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
  onUserThemesChanged(() => refetchUserThemes());

  onMount(() => {
    if (typeof window !== "undefined" && window.matchMedia) {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      const onChange = () => applyTheme(settingsResult()?.settings.theme);
      mq.addEventListener("change", onChange);
      onCleanup(() => mq.removeEventListener("change", onChange));
    }
  });

  onMount(async () => {
    try {
      const cliRepo = await getCliRepoArg();
      if (cliRepo) {
        workspace.openRepository(cliRepo);
      }
    } catch {
      // not running in tauri or no cli arg
    }
  });

  async function handleSaveSettings(next: Settings) {
    await saveSettings(next);
    await refetchSettings();
    applyTheme(next.theme);
  }

  async function handleRemoveBookmark(root: string) {
    await removeBookmark(root);
    await refetchBookmarks();
  }

  async function handleBrowse() {
    try {
      const selected = await pickFolder();
      if (selected) {
        setPathInput(selected);
        await openPath(selected);
      }
    } catch (e) {
      setOpenError(String(e));
    }
  }

  let unlistenChanged: (() => void) | undefined;
  let unlistenDegraded: (() => void) | undefined;
  let unlistenAskpass: (() => void) | undefined;
  onRepositoryChanged((root) => {
    invalidate(root);
    refetchOperationLog();
  }).then((un) => (unlistenChanged = un));
  onWatchDegraded((info) => {
    setWatchDegraded((prev) => ({ ...prev, [info.root]: info.reason }));
  }).then((un) => (unlistenDegraded = un));
  onAskpassPrompt((payload) => {
    setAskpassPrompt(payload);
  }).then((un) => (unlistenAskpass = un));
  onCleanup(() => {
    unlistenChanged?.();
    unlistenDegraded?.();
    unlistenAskpass?.();
  });

  async function handleAskpassSubmit(id: string, response: string) {
    try {
      await submitAskpassResponse(id, response);
    } finally {
      setAskpassPrompt(null);
    }
  }

  async function handleAskpassCancel(id: string) {
    try {
      await cancelAskpassResponse(id);
    } finally {
      setAskpassPrompt(null);
    }
  }

  async function handleCancelSync() {
    try {
      await cancelSyncNetworkOperation();
    } finally {
      setIsSyncing(false);
    }
  }

  onMount(() => {
    function onKeyDown(e: KeyboardEvent) {
      const target = e.target as HTMLElement | null;
      const isInput =
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable);

      const bindings = customKeybindings();
      const getShortcut = (id: string) => getEffectiveShortcut(id, bindings);

      if (matchesEvent(getShortcut("command-palette"), e)) {
        e.preventDefault();
        setShowCommandPalette((prev) => !prev);
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key === ",") {
        e.preventDefault();
        setShowSettingsModal((prev) => !prev);
        return;
      }

      if (showCommandPalette() || showKeybindingsModal() || showSettingsModal()) return;

      const groupId = workspace.activeGroup()?.id;
      if (groupId && e.ctrlKey && e.key === "Tab") {
        e.preventDefault();
        workspace.cycleTab(groupId, e.shiftKey ? -1 : 1);
        return;
      }

      if (isInput) return;

      if (matchesEvent(getShortcut("view-working-copy"), e)) {
        e.preventDefault();
        setMainView("working-copy");
      } else if (matchesEvent(getShortcut("view-history"), e)) {
        e.preventDefault();
        setMainView("history");
      } else if (matchesEvent(getShortcut("view-branches"), e)) {
        e.preventDefault();
        setMainView("branches");
      } else if (matchesEvent(getShortcut("view-tags"), e)) {
        e.preventDefault();
        setMainView("tags");
      } else if (matchesEvent(getShortcut("view-stashes"), e)) {
        e.preventDefault();
        setMainView("stashes");
      } else if (matchesEvent(getShortcut("view-reflog"), e)) {
        e.preventDefault();
        setReflogTargetRef("HEAD");
        setMainView("reflog");
      } else if (matchesEvent(getShortcut("fetch"), e)) {
        e.preventDefault();
        if (activeViewPath()) setShowFetchModal(true);
      } else if (matchesEvent(getShortcut("pull"), e)) {
        e.preventDefault();
        if (activeViewPath()) setShowPullModal(true);
      } else if (matchesEvent(getShortcut("push"), e)) {
        e.preventDefault();
        if (activeViewPath()) setShowPushModal(true);
      } else if (matchesEvent(getShortcut("refresh-state"), e)) {
        e.preventDefault();
        refreshAll();
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
    if (path) invalidate(path, { submodules: true });
    refetchOperationLog();
  }

  async function cancelWorkingCopyScan() {
    try {
      await cancelWorkingCopyStatus();
    } catch (_) {}
  }

  const commandPaletteItems = createMemo<CommandPaletteItem[]>(() => {
    const items: CommandPaletteItem[] = [];
    const bindings = customKeybindings();
    const getShortcut = (id: string) => getEffectiveShortcut(id, bindings);
    const activePath = activeViewPath();
    const hasActiveRepo = !!activePath;

    // Navigation
    items.push({
      id: "nav-working-copy",
      title: "Go to Working Copy",
      category: "Navigation",
      shortcut: getShortcut("view-working-copy"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => setMainView("working-copy"),
    });
    items.push({
      id: "nav-history",
      title: "Go to History",
      category: "Navigation",
      shortcut: getShortcut("view-history"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => setMainView("history"),
    });
    items.push({
      id: "nav-submodules",
      title: "Go to Submodules",
      category: "Navigation",
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => setMainView("submodules"),
    });
    items.push({
      id: "nav-branches",
      title: "Go to Branches",
      category: "Navigation",
      shortcut: getShortcut("view-branches"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => setMainView("branches"),
    });
    items.push({
      id: "nav-tags",
      title: "Go to Tags",
      category: "Navigation",
      shortcut: getShortcut("view-tags"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => setMainView("tags"),
    });
    items.push({
      id: "nav-stashes",
      title: "Go to Stashes",
      category: "Navigation",
      shortcut: getShortcut("view-stashes"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => setMainView("stashes"),
    });
    items.push({
      id: "nav-reflog",
      title: "Go to Reflog",
      category: "Navigation",
      shortcut: getShortcut("view-reflog"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => {
        setReflogTargetRef("HEAD");
        setMainView("reflog");
      },
    });

    // Sync
    items.push({
      id: "sync-fetch",
      title: "Fetch Remotes",
      category: "Sync",
      shortcut: getShortcut("fetch"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => setShowFetchModal(true),
    });
    items.push({
      id: "sync-pull",
      title: "Pull from Upstream",
      category: "Sync",
      shortcut: getShortcut("pull"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => setShowPullModal(true),
    });
    items.push({
      id: "sync-push",
      title: "Push to Remote",
      category: "Sync",
      shortcut: getShortcut("push"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: () => setShowPushModal(true),
    });

    // Working Copy
    const wc = workingCopy();
    const hasStaged =
      wc &&
      isKnown(wc) &&
      wc.value.changed.some((e) => e.staged !== "Unmodified");
    const hasUnstaged =
      wc &&
      isKnown(wc) &&
      (wc.value.changed.some((e) => e.unstaged !== "Unmodified") ||
        wc.value.untracked.length > 0);

    items.push({
      id: "wc-stage-all",
      title: "Stage All Changes",
      category: "Working Copy",
      shortcut: getShortcut("stage-all"),
      available: hasActiveRepo && !!hasUnstaged,
      unavailableReason: !hasActiveRepo
        ? "No active repository open"
        : !hasUnstaged
        ? "No unstaged changes"
        : undefined,
      onExecute: () => {
        if (wc && isKnown(wc)) {
          const unstagedPaths = wc.value.changed
            .filter((e) => e.unstaged !== "Unmodified")
            .map((e) => e.path);
          const allPaths = [...unstagedPaths, ...wc.value.untracked];
          void stagePaths(allPaths);
        }
      },
    });
    items.push({
      id: "wc-unstage-all",
      title: "Unstage All Changes",
      category: "Working Copy",
      available: hasActiveRepo && !!hasStaged,
      unavailableReason: !hasActiveRepo
        ? "No active repository open"
        : !hasStaged
        ? "No staged changes"
        : undefined,
      onExecute: () => {
        if (wc && isKnown(wc)) {
          const stagedPaths = wc.value.changed
            .filter((e) => e.staged !== "Unmodified")
            .map((e) => e.path);
          void unstagePaths(stagedPaths);
        }
      },
    });
    items.push({
      id: "wc-discard-all",
      title: "Discard All Changes",
      category: "Working Copy",
      shortcut: getShortcut("discard-all"),
      isDestructive: true,
      available: hasActiveRepo && !!hasUnstaged,
      unavailableReason: !hasActiveRepo
        ? "No active repository open"
        : !hasUnstaged
        ? "No unstaged changes"
        : undefined,
      onExecute: () => {
        setMainView("working-copy");
      },
    });

    // Repositories
    for (const b of bookmarks() ?? []) {
      items.push({
        id: `repo-bookmark-${b.root}`,
        title: `Open Repository: ${b.root}`,
        category: "Repositories",
        available: true,
        onExecute: () => activateRoot(b.root),
      });
    }

    for (const g of workspace.state().groups) {
      if (!bookmarks()?.some((b) => b.root === g.rootPath)) {
        items.push({
          id: `repo-group-${g.id}`,
          title: `Switch to Group: ${g.rootPath}`,
          category: "Repositories",
          available: true,
          onExecute: () => workspace.setActiveGroup(g.id),
        });
      }
    }

    // General
    items.push({
      id: "general-refresh",
      title: "Refresh Repository State",
      category: "General",
      shortcut: getShortcut("refresh-state"),
      available: hasActiveRepo,
      unavailableReason: hasActiveRepo ? undefined : "No active repository open",
      onExecute: refreshAll,
    });
    items.push({
      id: "general-settings",
      title: "Preferences: Open Settings",
      category: "General",
      shortcut: "Ctrl+,",
      available: true,
      onExecute: () => setShowSettingsModal(true),
    });
    items.push({
      id: "general-shortcuts",
      title: "Keyboard Shortcuts",
      category: "General",
      available: true,
      onExecute: () => setShowKeybindingsModal(true),
    });
    items.push({
      id: "general-toggle-detail",
      title: detailCollapsed() ? "Show Detail Panel" : "Hide Detail Panel",
      category: "General",
      available: true,
      onExecute: () => setDetailCollapsed(!detailCollapsed()),
    });

    return items;
  });

  return (
    <div class="app-shell">
      <header class="app-titlebar" role="banner" data-tauri-drag-region>
        <div class="app-title-left">
          <div class="app-title-brand">
            <img src="/logo.png" alt="GitTree" class="app-title-logo app-title-logo-light" />
            <img src="/logo-dark.png" alt="" class="app-title-logo app-title-logo-dark" />
            <span class="brand-text">GitTree</span>
          </div>
          <Show when={workspace.activeGroup()}>
            {(group) => {
              const repoBasename = () => {
                const p = group().rootPath;
                return p.split("/").filter(Boolean).pop() || p;
              };
              return (
                <div class="app-title-repo">
                  <span class="app-title-separator">/</span>
                  <span class="repo-name" title={group().rootPath}>{repoBasename()}</span>
                  <Show when={repoState()?.branch}>
                    <span class="repo-branch-pill" title={`Branch: ${branchLabel(repoState()!.branch)}`}>
                      <Icon name="branch" size={12} />
                      <span class="branch-pill-name">{branchLabel(repoState()!.branch)}</span>
                    </span>
                  </Show>
                </div>
              );
            }}
          </Show>
        </div>

        <div class="app-title-center">
          <Show when={workspace.activeGroup() && repoState()}>
            <SyncToolbar
              onOpenFetch={() => setShowFetchModal(true)}
              onOpenPull={() => setShowPullModal(true)}
              onOpenPush={() => setShowPushModal(true)}
              aheadCount={aheadBehind()?.ahead}
              behindCount={aheadBehind()?.behind}
              isSyncing={isSyncing()}
              onCancelSync={handleCancelSync}
            />
          </Show>
        </div>

        <div class="app-title-right">
          <Show when={workspace.activeGroup()}>
            <button
              type="button"
              class="icon-btn"
              onClick={refreshAll}
              title="Refresh (Ctrl+R)"
              aria-label="Refresh"
            >
              <Icon name="refresh" size={14} />
            </button>
          </Show>
          <button
            type="button"
            class="icon-btn"
            onClick={() => setShowCommandPalette(true)}
            title="Command Palette (Ctrl+K)"
            aria-label="Command Palette"
          >
            <Icon name="search" size={14} />
          </button>
          <button
            type="button"
            class="icon-btn"
            onClick={() => setShowKeybindingsModal(true)}
            title="Keyboard Shortcuts"
            aria-label="Keyboard Shortcuts"
          >
            <Icon name="terminal" size={14} />
          </button>
          <button
            type="button"
            class="icon-btn"
            onClick={() => setShowSettingsModal(true)}
            title="Preferences (Ctrl+,)"
            aria-label="Preferences"
          >
            <Icon name="settings" size={14} />
          </button>
          <Show when={workspace.activeGroup()}>
            <button
              type="button"
              class={`icon-btn ${!detailCollapsed() ? "active" : ""}`}
              onClick={() => setDetailCollapsed(!detailCollapsed())}
              title={detailCollapsed() ? "Show Operation Log" : "Hide Operation Log"}
              aria-label="Operation Log"
            >
              <Icon name="list" size={14} />
            </button>
          </Show>
          <div class="window-controls">
            <button
              type="button"
              class="window-ctrl-btn"
              onClick={() => void minimizeWindow()}
              title="Minimize"
              aria-label="Minimize"
            >
              <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor"/></svg>
            </button>
            <button
              type="button"
              class="window-ctrl-btn"
              onClick={() => void toggleMaximizeWindow()}
              title="Maximize"
              aria-label="Maximize"
            >
              <svg width="9" height="9" viewBox="0 0 9 9"><rect x="0.5" y="0.5" width="8" height="8" fill="none" stroke="currentColor"/></svg>
            </button>
            <button
              type="button"
              class="window-ctrl-btn close-btn"
              onClick={() => void closeWindow()}
              title="Close"
              aria-label="Close"
            >
              <svg width="9" height="9" viewBox="0 0 9 9"><line x1="0" y1="0" x2="9" y2="9" stroke="currentColor"/><line x1="9" y1="0" x2="0" y2="9" stroke="currentColor"/></svg>
            </button>
          </div>
        </div>
      </header>

      <Show when={(userThemesResult()?.errors.length ?? 0) > 0}>
        <div class="user-theme-error-banner" role="alert">
          <Icon name="warning" size={14} />
          <span>Theme error:</span>
          <For each={userThemesResult()?.errors}>
            {(err) => (
              <span class="user-theme-error-item">
                {err.file}{err.line ? `:${err.line}` : ""}: {err.message}
              </span>
            )}
          </For>
        </div>
      </Show>

      <Show when={workspace.state().groups.length > 1}>
        <GroupSwitcher
          groups={workspace.state().groups}
          activeGroupId={workspace.state().activeGroupId}
          onSelect={(id) => workspace.setActiveGroup(id)}
          onClose={closeGroup}
        />
      </Show>
      <Show when={workspace.activeGroup()}>
        {(group) => (
          <Show when={group().tabs.length > 1}>
            <TabStrip
              tabs={group().tabs}
              activeTabId={group().activeTabId}
              onSelect={(tabId) => workspace.setActiveTab(group().id, tabId)}
              onClose={(tabId) => workspace.closeTab(group().id, tabId)}
            />
          </Show>
        )}
      </Show>

      <Show when={workspace.activeGroup() && watchDegraded()[workspace.activeGroup()!.id]}>
        <div class="watch-degraded-banner">Filesystem watching degraded: {watchDegraded()[workspace.activeGroup()!.id]}</div>
      </Show>

      <div class="app-body">
        <Show when={!sidebarCollapsed()}>
          <nav
            class="pane-sidebar"
            style={{ width: `${clamp(sidebarWidth(), 200, 480)}px` }}
            aria-label="Repository navigation"
          >
            <RepositorySidebar
              root={activeViewPath()}
              repoState={repoState() ?? null}
              activeView={mainView()}
              onSelectView={(v) => {
                if (v === "history") setHistoryScope(undefined);
                if (v === "reflog") setReflogTargetRef("HEAD");
                setMainView(v);
              }}
              workingCopyStatus={workingCopy() ?? null}
              submoduleCount={submodules()?.submodules.length ?? 0}
              bookmarks={bookmarks() ?? []}
              isBookmarked={Boolean(workspace.activeGroup() && bookmarks()?.some((b) => b.root === workspace.activeGroup()!.rootPath))}
              onToggleBookmark={() => {
                const group = workspace.activeGroup();
                if (group) void toggleBookmark(group.rootPath);
              }}
              onOpenRepo={(path) => void openPath(path)}
              onBrowse={() => void handleBrowse()}
              pathInput={pathInput()}
              onPathInput={setPathInput}
              openError={openError()}
              offerInitAt={offerInitAt()}
              onConfirmInit={(p) => void confirmInit(p)}
              onCancelInit={() => setOfferInitAt(null)}
              setPathInputEl={setPathInputEl}
            />
          </nav>
          <Resizer label="Resize sidebar" onResize={(d) => setSidebarWidth(clamp(sidebarWidth() + d, 200, 480))} />
        </Show>

        <main class="pane-content">
          <button
            class="collapse-toggle"
            aria-expanded={!sidebarCollapsed()}
            onClick={() => setSidebarCollapsed(!sidebarCollapsed())}
          >
            {sidebarCollapsed() ? "Show sidebar" : "Hide sidebar"}
          </button>

          <Show
            when={workspace.activeGroup()}
            fallback={
              <div class="empty-workspace">
                <Icon name="folder-open" size={48} class="empty-workspace-icon" />
                <div class="empty-workspace-title">No repository open</div>
                <div class="empty-workspace-desc">
                  Open an existing Git repository from disk, or choose an action from the command palette.
                </div>
                <div class="empty-workspace-actions">
                  <button type="button" class="empty-workspace-btn" onClick={() => void handleBrowse()}>
                    <Icon name="folder-open" size={16} />
                    <span>Open Repository…</span>
                  </button>
                  <button
                    type="button"
                    class="empty-workspace-btn-secondary"
                    onClick={() => setShowCommandPalette(true)}
                  >
                    <Icon name="search" size={16} />
                    <span>Command Palette (Ctrl+K)</span>
                  </button>
                </div>
              </div>
            }
          >
            <Show when={integrationNotice()}>
              {(msg) => (
                <div class="integration-notice" style={{ padding: "var(--space-2) var(--space-4)", "background-color": "var(--color-bg-subtle)", "border-bottom": "1px solid var(--color-border-subtle)", display: "flex", "align-items": "center", "justify-content": "space-between" }}>
                  <span>{msg()}</span>
                  <button class="collapse-toggle" onClick={() => setIntegrationNotice(null)}>Dismiss</button>
                </div>
              )}
            </Show>
            <Show when={activeOperation()}>
              {(op) => (
                <ActiveOperationBanner
                  root={activeViewPath()!}
                  operation={op()}
                  onOperationUpdated={() => invalidate(activeViewPath()!)}
                  onOperationAborted={(outcome) => {
                    setIntegrationNotice(outcome.summary);
                    invalidate(activeViewPath()!);
                  }}
                  onNavigateWorkingCopy={() => setMainView("working-copy")}
                />
              )}
            </Show>
            <Show when={mainView() === "branches"}>
              <BranchesView
                root={activeViewPath()!}
                onCheckoutSuccess={() => {
                  const path = activeViewPath();
                  if (path) invalidate(path);
                }}
                onNavigateWorkingCopy={() => setMainView("working-copy")}
                onOpenReflog={(branch) => {
                  setReflogTargetRef(branch);
                  setMainView("reflog");
                }}
              />
            </Show>
            <Show when={mainView() === "tags"}>
              <TagsView
                root={activeViewPath()!}
                onCheckoutSuccess={() => {
                  const path = activeViewPath();
                  if (path) invalidate(path);
                }}
              />
            </Show>
            <Show when={mainView() === "stashes"}>
              <StashesView
                root={activeViewPath()!}
                onNavigateWorkingCopy={() => setMainView("working-copy")}
              />
            </Show>
            <Show when={mainView() === "reflog"}>
              <ReflogView
                root={activeViewPath()!}
                initialRef={reflogTargetRef()}
                onSelectCommit={() => {
                  setHistoryScope(undefined);
                  setMainView("history");
                }}
                onResetSuccess={(outcome) => {
                  setIntegrationNotice(outcome.summary);
                  invalidate(activeViewPath()!);
                }}
              />
            </Show>
            <Show when={mainView() === "history"}>
              <HistoryView
                root={activeViewPath()!}
                hasUncommittedChanges={hasUncommittedChanges()}
                onSelectUncommitted={() => setMainView("working-copy")}
                initialScope={historyScope()}
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
                    isScanning={workingCopy.loading}
                    onCancelScan={cancelWorkingCopyScan}
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
                    onViewHistory={(path) => {
                      setHistoryScope({ kind: "Path", path });
                      setMainView("history");
                    }}
                  />
                )}
              </Show>
            </Show>
            <Show when={mainView() === "submodules"}>
              <Show when={submodules()}>
                {(submodData) => (
                  <SubmoduleMatrix
                    root={workspace.activeGroup()?.rootPath}
                    submodules={submodData().submodules}
                    malformedEntries={submodData().malformed_entries}
                    onDrillIn={drillIntoSubmodule}
                    onRefreshNeeded={() => refetchSubmodules()}
                  />
                )}
              </Show>
              <Show when={submodules.loading}>
                <p class="text-muted" style={{ padding: "var(--space-4)" }}>Loading submodules…</p>
              </Show>
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

      <Show when={conflictMarkerRefusal()}>
        {(refusal) => (
          <ConflictMarkerGuardModal
            file={refusal().file}
            markerLines={refusal().marker_lines}
            previewLines={refusal().preview_lines}
            onCancel={() => setConflictMarkerRefusal(null)}
            onConfirmOverride={async () => {
              const pathsToStage = refusal().paths;
              setConflictMarkerRefusal(null);
              await stagePaths(pathsToStage, true);
            }}
          />
        )}
      </Show>

      <Show when={showFetchModal() && activeViewPath()}>
        <FetchModal
          root={activeViewPath()!}
          onClose={() => setShowFetchModal(false)}
          onSuccess={() => invalidate(activeViewPath()!)}
        />
      </Show>

      <Show when={showPullModal() && activeViewPath()}>
        <PullModal
          root={activeViewPath()!}
          currentBranch={repoState()?.branch ? branchLabel(repoState()!.branch) : undefined}
          onClose={() => setShowPullModal(false)}
          onSuccess={() => invalidate(activeViewPath()!)}
          onConflict={() => {
            setShowPullModal(false);
            setMainView("working-copy");
            invalidate(activeViewPath()!);
          }}
        />
      </Show>

      <Show when={showPushModal() && activeViewPath()}>
        <PushModal
          root={activeViewPath()!}
          currentBranch={repoState()?.branch ? branchLabel(repoState()!.branch) : undefined}
          onClose={() => setShowPushModal(false)}
          onSuccess={() => invalidate(activeViewPath()!)}
          onOpenPull={() => setShowPullModal(true)}
        />
      </Show>

      <Show when={askpassPrompt()}>
        {(prompt) => (
          <AskpassModal
            prompt={prompt()}
            onSubmit={handleAskpassSubmit}
            onCancel={handleAskpassCancel}
          />
        )}
      </Show>

      <CommandPalette
        open={showCommandPalette()}
        onClose={() => setShowCommandPalette(false)}
        items={commandPaletteItems()}
      />

      <KeybindingsModal
        open={showKeybindingsModal()}
        onClose={() => setShowKeybindingsModal(false)}
        customBindings={customKeybindings()}
        onSaveBindings={handleSaveBindings}
      />

      <Show when={activeCommandError()}>
        {(err) => (
          <CommandErrorModal
            open={true}
            title={err().title}
            command={err().command}
            exitStatus={err().exitStatus}
            stderr={err().stderr}
            onClose={() => setActiveCommandError(null)}
          />
        )}
      </Show>

      <SettingsModal
        isOpen={showSettingsModal()}
        onClose={() => setShowSettingsModal(false)}
        settings={settingsResult()?.settings ?? { concurrency: 8, theme: null }}
        onSaveSettings={handleSaveSettings}
        userThemesResult={userThemesResult()}
        bookmarks={bookmarks() ?? []}
        onRemoveBookmark={handleRemoveBookmark}
        onOpenBookmark={(root: string) => {
          setShowSettingsModal(false);
          void openPath(root);
        }}
      />
    </div>
  );
};
