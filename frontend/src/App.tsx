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
import { branchLabel, isKnown, type AskpassPromptPayload, type Bookmark, type RepositoryState, type Resolved, type Settings, type SubmoduleMatrixResult, type WorkingCopyStatus } from "./api/types";
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
import { RepositoryStatus } from "./features/repository/RepositoryStatus";
import { SubmoduleMatrix } from "./features/submodules/SubmoduleMatrix";
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
  const [mainView, setMainView] = createSignal<"working-copy" | "history" | "branches" | "tags" | "stashes" | "reflog">("working-copy");
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

  async function setTheme(theme: string) {
    const current = settingsResult()?.settings ?? { concurrency: 8, theme: null };
    const next = { ...current, theme: theme === "system" ? null : theme };
    await saveSettings(next);
    await refetchSettings();
    applyTheme(next.theme);
  }

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
    if (path) invalidate(path);
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
      <header class="app-titlebar" role="banner">
        <div class="app-title-brand">
          <img src="/logo.png" alt="GitTree" class="app-title-logo app-title-logo-light" />
          <img src="/logo-dark.png" alt="" class="app-title-logo app-title-logo-dark" />
          <span>GitTree</span>
        </div>
        <button
          type="button"
          class="collapse-toggle"
          onClick={() => setShowCommandPalette(true)}
          title="Command Palette (Ctrl+K)"
          aria-label="Command Palette"
        >
          <Icon name="search" size={14} />
          <span>Palette</span>
        </button>
        <button
          type="button"
          class="collapse-toggle"
          onClick={() => setShowKeybindingsModal(true)}
          title="Keyboard Shortcuts"
          aria-label="Keyboard Shortcuts"
        >
          <Icon name="terminal" size={14} />
          <span>Shortcuts</span>
        </button>
        <button
          type="button"
          class="collapse-toggle"
          onClick={() => setShowSettingsModal(true)}
          title="Preferences (Ctrl+,)"
          aria-label="Preferences"
        >
          <Icon name="settings" size={14} />
          <span>Settings</span>
        </button>
        <Show when={workspace.activeGroup()}>
          <button class="collapse-toggle" onClick={refreshAll} title="Refresh (Ctrl+R)">
            <Icon name="refresh" size={14} />
            <span>Refresh</span>
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
            <option value="system">Follow Desktop</option>
            <option value="light">Light (Built-in)</option>
            <option value="dark">Dark (Built-in)</option>
            <Show when={(userThemesResult()?.themes.length ?? 0) > 0}>
              <optgroup label="User Themes">
                <For each={userThemesResult()?.themes}>
                  {(t) => <option value={t.id}>{t.name}</option>}
                </For>
              </optgroup>
            </Show>
          </select>
        </label>
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
              <div class="open-repo-actions">
                <button type="button" onClick={() => void openPath(pathInput())}>Open</button>
                <button
                  type="button"
                  onClick={() => void handleBrowse()}
                  title="Browse local folder"
                  aria-label="Browse local folder"
                >
                  <Icon name="folder-open" size={14} />
                  <span>Browse…</span>
                </button>
              </div>
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
            <Show when={repoState()}>
              {(state) => (
                <RepositoryStatus
                  state={state()}
                  onOpenBranches={() => setMainView("branches")}
                  onOpenTags={() => setMainView("tags")}
                  onOpenStashes={() => setMainView("stashes")}
                  onOpenReflog={() => {
                    setReflogTargetRef("HEAD");
                    setMainView("reflog");
                  }}
                  onOpenFetch={() => setShowFetchModal(true)}
                  onOpenPull={() => setShowPullModal(true)}
                  onOpenPush={() => setShowPushModal(true)}
                  isSyncing={isSyncing()}
                  onCancelSync={handleCancelSync}
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
                onClick={() => {
                  setHistoryScope(undefined);
                  setMainView("history");
                }}
              >
                History
              </button>
              <button
                class="collapse-toggle"
                aria-pressed={mainView() === "branches"}
                onClick={() => setMainView("branches")}
              >
                Branches
              </button>
              <button
                class="collapse-toggle"
                aria-pressed={mainView() === "tags"}
                onClick={() => setMainView("tags")}
              >
                Tags
              </button>
              <button
                class="collapse-toggle"
                aria-pressed={mainView() === "stashes"}
                onClick={() => setMainView("stashes")}
              >
                Stashes
              </button>
              <button
                class="collapse-toggle"
                aria-pressed={mainView() === "reflog"}
                onClick={() => {
                  setReflogTargetRef("HEAD");
                  setMainView("reflog");
                }}
              >
                Reflog
              </button>
            </div>
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
            <Show when={submodules() && (submodules()!.submodules.length > 0 || submodules()!.malformed_entries.length > 0)}>
              <SubmoduleMatrix
                root={workspace.activeGroup()?.rootPath}
                submodules={submodules()!.submodules}
                malformedEntries={submodules()!.malformed_entries}
                onDrillIn={drillIntoSubmodule}
                onRefreshNeeded={() => refetchSubmodules()}
              />
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
