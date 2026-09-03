import { describe, expect, it } from "vitest";
import {
  activeGroup,
  activeTab,
  closeGroup,
  closeTab,
  cycleTab,
  emptyWorkspace,
  openRepository,
  openSubmoduleTab,
  setActiveGroup,
} from "./model";

describe("openRepository", () => {
  it("creates a new group and activates it", () => {
    const state = openRepository(emptyWorkspace(), "/repos/a");
    expect(state.groups).toHaveLength(1);
    expect(state.activeGroupId).toBe("/repos/a");
    expect(activeGroup(state)?.tabs).toEqual([{ id: "/repos/a", path: "/repos/a", kind: "repository", label: "a" }]);
  });

  it("opening a second repository does not close the first", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = openRepository(state, "/repos/b");
    expect(state.groups.map((g) => g.id)).toEqual(["/repos/a", "/repos/b"]);
    expect(state.activeGroupId).toBe("/repos/b");
  });

  it("reopening an already-open repository activates it rather than duplicating", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = openRepository(state, "/repos/b");
    state = openRepository(state, "/repos/a");
    expect(state.groups).toHaveLength(2);
    expect(state.activeGroupId).toBe("/repos/a");
  });
});

describe("switching groups preserves each one's state", () => {
  it("group A's active tab survives switching to B and back", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = openSubmoduleTab(state, "/repos/a", "/repos/a/sub1", "sub1");
    state = openRepository(state, "/repos/b");
    state = setActiveGroup(state, "/repos/a");
    expect(activeTab(state)?.id).toBe("/repos/a/sub1");
  });
});

describe("closeGroup", () => {
  it("closing one group does not affect others", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = openRepository(state, "/repos/b");
    state = closeGroup(state, "/repos/a");
    expect(state.groups.map((g) => g.id)).toEqual(["/repos/b"]);
    expect(state.activeGroupId).toBe("/repos/b");
  });

  it("closing the active group activates another open group", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = openRepository(state, "/repos/b");
    state = closeGroup(state, "/repos/b");
    expect(state.activeGroupId).toBe("/repos/a");
  });

  it("closing the last group leaves no active group", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = closeGroup(state, "/repos/a");
    expect(state.activeGroupId).toBeNull();
    expect(state.groups).toHaveLength(0);
  });
});

describe("submodule drill-in as tabs", () => {
  it("multiple submodules stay open as separate tabs", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = openSubmoduleTab(state, "/repos/a", "/repos/a/despachomanager", "despachomanager");
    state = openSubmoduleTab(state, "/repos/a", "/repos/a/devicepolling", "devicepolling");
    const group = activeGroup(state)!;
    expect(group.tabs.map((t) => t.id)).toEqual(["/repos/a", "/repos/a/despachomanager", "/repos/a/devicepolling"]);
    expect(group.activeTabId).toBe("/repos/a/devicepolling");
  });

  it("reopening an already-open submodule activates its tab instead of duplicating", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = openSubmoduleTab(state, "/repos/a", "/repos/a/sub1", "sub1");
    state = openSubmoduleTab(state, "/repos/a", "/repos/a/sub2", "sub2");
    state = openSubmoduleTab(state, "/repos/a", "/repos/a/sub1", "sub1");
    const group = activeGroup(state)!;
    expect(group.tabs).toHaveLength(3);
    expect(group.activeTabId).toBe("/repos/a/sub1");
  });

  it("the repository's own tab cannot be closed", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = closeTab(state, "/repos/a", "/repos/a");
    expect(activeGroup(state)?.tabs).toHaveLength(1);
  });

  it("closing the active tab falls back to the previous one", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = openSubmoduleTab(state, "/repos/a", "/repos/a/sub1", "sub1");
    state = openSubmoduleTab(state, "/repos/a", "/repos/a/sub2", "sub2");
    state = closeTab(state, "/repos/a", "/repos/a/sub2");
    expect(activeTab(state)?.id).toBe("/repos/a/sub1");
  });
});

describe("cycleTab", () => {
  it("wraps around in both directions", () => {
    let state = openRepository(emptyWorkspace(), "/repos/a");
    state = openSubmoduleTab(state, "/repos/a", "/repos/a/sub1", "sub1");
    state = cycleTab(state, "/repos/a", 1);
    expect(activeTab(state)?.id).toBe("/repos/a"); // wrapped past the end
    state = cycleTab(state, "/repos/a", -1);
    expect(activeTab(state)?.id).toBe("/repos/a/sub1"); // wrapped past the start
  });
});
