// State transitions for the workspace: a list of groups, each with its own tab list.

export type TabKind = "repository" | "submodule";

export type Tab = {
  id: string; // the tab's repository/submodule path
  path: string;
  kind: TabKind;
  label: string;
};

export type WorkspaceGroup = {
  id: string; // the group's repository root path
  rootPath: string;
  rootLabel: string;
  tabs: Tab[];
  activeTabId: string;
};

export type WorkspaceState = {
  groups: WorkspaceGroup[];
  activeGroupId: string | null;
};

export function emptyWorkspace(): WorkspaceState {
  return { groups: [], activeGroupId: null };
}

function basename(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const parts = trimmed.split("/");
  return parts[parts.length - 1] || trimmed;
}

/** Opens (or activates, if already open) a repository as its own group. */
export function openRepository(state: WorkspaceState, root: string): WorkspaceState {
  const existing = state.groups.find((g) => g.id === root);
  if (existing) {
    return { ...state, activeGroupId: root };
  }
  const rootTab: Tab = { id: root, path: root, kind: "repository", label: basename(root) };
  const group: WorkspaceGroup = {
    id: root,
    rootPath: root,
    rootLabel: rootTab.label,
    tabs: [rootTab],
    activeTabId: rootTab.id,
  };
  return { groups: [...state.groups, group], activeGroupId: root };
}

/** Closes a group entirely. Activates another open group, or none. */
export function closeGroup(state: WorkspaceState, groupId: string): WorkspaceState {
  const groups = state.groups.filter((g) => g.id !== groupId);
  const activeGroupId =
    state.activeGroupId === groupId ? (groups.length > 0 ? groups[groups.length - 1].id : null) : state.activeGroupId;
  return { groups, activeGroupId };
}

export function setActiveGroup(state: WorkspaceState, groupId: string): WorkspaceState {
  if (!state.groups.some((g) => g.id === groupId)) return state;
  return { ...state, activeGroupId: groupId };
}

function mapGroup(state: WorkspaceState, groupId: string, fn: (g: WorkspaceGroup) => WorkspaceGroup): WorkspaceState {
  return { ...state, groups: state.groups.map((g) => (g.id === groupId ? fn(g) : g)) };
}

/** Opens a submodule as a tab within its superproject's group. Activates
 * the existing tab instead of duplicating it if already open. */
export function openSubmoduleTab(state: WorkspaceState, groupId: string, submodulePath: string, label: string): WorkspaceState {
  return mapGroup(state, groupId, (g) => {
    const existing = g.tabs.find((t) => t.id === submodulePath);
    if (existing) return { ...g, activeTabId: existing.id };
    const tab: Tab = { id: submodulePath, path: submodulePath, kind: "submodule", label };
    return { ...g, tabs: [...g.tabs, tab], activeTabId: tab.id };
  });
}

/** Closes a tab. The group's own repository tab (tabs[0]) cannot be
 * closed individually — close the group instead. */
export function closeTab(state: WorkspaceState, groupId: string, tabId: string): WorkspaceState {
  return mapGroup(state, groupId, (g) => {
    if (g.tabs[0]?.id === tabId) return g;
    const tabs = g.tabs.filter((t) => t.id !== tabId);
    const activeTabId = g.activeTabId === tabId ? tabs[tabs.length - 1].id : g.activeTabId;
    return { ...g, tabs, activeTabId };
  });
}

export function setActiveTab(state: WorkspaceState, groupId: string, tabId: string): WorkspaceState {
  return mapGroup(state, groupId, (g) => (g.tabs.some((t) => t.id === tabId) ? { ...g, activeTabId: tabId } : g));
}

export function activeGroup(state: WorkspaceState): WorkspaceGroup | null {
  return state.groups.find((g) => g.id === state.activeGroupId) ?? null;
}

export function activeTab(state: WorkspaceState): Tab | null {
  const group = activeGroup(state);
  if (!group) return null;
  return group.tabs.find((t) => t.id === group.activeTabId) ?? null;
}

/** Moves the active tab within `groupId` forward or backward, wrapping. */
export function cycleTab(state: WorkspaceState, groupId: string, direction: 1 | -1): WorkspaceState {
  return mapGroup(state, groupId, (g) => {
    const index = g.tabs.findIndex((t) => t.id === g.activeTabId);
    if (index === -1) return g;
    const next = (index + direction + g.tabs.length) % g.tabs.length;
    return { ...g, activeTabId: g.tabs[next].id };
  });
}
