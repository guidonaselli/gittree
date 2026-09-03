import { createSignal } from "solid-js";
import * as ws from "./model";

const STORAGE_KEY = "gittree.workspace";

function readStored(): ws.WorkspaceState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as ws.WorkspaceState) : ws.emptyWorkspace();
  } catch {
    return ws.emptyWorkspace();
  }
}

function writeStored(state: ws.WorkspaceState) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // storage unavailable; workspace state simply stops persisting
  }
}

/** Persisted multi-repository workspace: groups (one per open repository)
 * with their own tab strips. See workspace/model.ts for the transitions. */
export function useWorkspace() {
  const [state, setState] = createSignal<ws.WorkspaceState>(readStored());

  function apply(fn: (s: ws.WorkspaceState) => ws.WorkspaceState) {
    const next = fn(state());
    setState(next);
    writeStored(next);
  }

  return {
    state,
    activeGroup: () => ws.activeGroup(state()),
    activeTab: () => ws.activeTab(state()),
    openRepository: (root: string) => apply((s) => ws.openRepository(s, root)),
    closeGroup: (groupId: string) => apply((s) => ws.closeGroup(s, groupId)),
    setActiveGroup: (groupId: string) => apply((s) => ws.setActiveGroup(s, groupId)),
    openSubmoduleTab: (groupId: string, path: string, label: string) =>
      apply((s) => ws.openSubmoduleTab(s, groupId, path, label)),
    closeTab: (groupId: string, tabId: string) => apply((s) => ws.closeTab(s, groupId, tabId)),
    setActiveTab: (groupId: string, tabId: string) => apply((s) => ws.setActiveTab(s, groupId, tabId)),
    cycleTab: (groupId: string, direction: 1 | -1) => apply((s) => ws.cycleTab(s, groupId, direction)),
  };
}
