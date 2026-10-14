import { type Component, For, Show, createMemo, createSignal } from "solid-js";
import {
  branchLabel,
  isKnown,
  type Bookmark,
  type BranchEntry,
  type RepositoryState,
  type Resolved,
  type SubmoduleState,
  type WorkingCopyStatus,
} from "../../api/types";
import {
  isDirty,
  isDetached,
  gitlinkAheadBehind,
} from "../submodules/submodule-helpers";
import { Icon } from "../../ui/Icon";
import { useSidebarSectionsCollapsed } from "../../layout/persisted-layout";
import "./repository.css";

export type NavView =
  | "working-copy"
  | "history"
  | "branches"
  | "tags"
  | "stashes"
  | "reflog"
  | "submodules";

export interface RepositorySidebarProps {
  root: string | null;
  repoState: RepositoryState | null;
  activeView: NavView;
  onSelectView: (view: NavView) => void;
  workingCopyStatus: Resolved<WorkingCopyStatus> | null;
  submoduleCount: number;
  submodules?: SubmoduleState[];
  branches?: BranchEntry[];
  onCheckoutBranch?: (name: string) => void;
  onDrillInSubmodule?: (path: string, name: string) => void;
  bookmarks: Bookmark[];
  isBookmarked: boolean;
  onToggleBookmark: () => void;
  onOpenRepo: (path: string) => void;
  onBrowse: () => void;
  pathInput: string;
  onPathInput: (path: string) => void;
  openError: string | null;
  offerInitAt: string | null;
  onConfirmInit: (path: string) => void;
  onCancelInit: () => void;
  setPathInputEl?: (el: HTMLInputElement) => void;
}

export const RepositorySidebar: Component<RepositorySidebarProps> = (props) => {
  const [sectionsCollapsed, setSectionsCollapsed] = useSidebarSectionsCollapsed();
  const [searchFilter, setSearchFilter] = createSignal("");

  const repoName = createMemo(() => {
    if (!props.root) return "";
    const parts = props.root.split("/").filter(Boolean);
    return parts[parts.length - 1] || props.root;
  });

  const branchName = createMemo(() => {
    if (!props.repoState?.branch) return "HEAD";
    return branchLabel(props.repoState.branch);
  });

  const aheadBehind = createMemo(() => {
    if (!props.repoState || !isKnown(props.repoState.ahead_behind)) return null;
    return props.repoState.ahead_behind.value;
  });

  const workingCopyCount = createMemo(() => {
    if (!props.workingCopyStatus || !isKnown(props.workingCopyStatus)) return 0;
    const v = props.workingCopyStatus.value;
    return v.changed.length + v.untracked.length + v.conflicted.length;
  });

  const stashCount = createMemo(() => {
    if (!props.repoState || !isKnown(props.repoState.stash_count)) return 0;
    return props.repoState.stash_count.value;
  });

  const localBranches = createMemo(() => {
    const list = props.branches?.filter((b) => !b.is_remote) ?? [];
    const q = searchFilter().toLowerCase().trim();
    if (!q) return list;
    return list.filter((b) => b.name.toLowerCase().includes(q));
  });

  const filteredSubmodules = createMemo(() => {
    const list = props.submodules ?? [];
    const q = searchFilter().toLowerCase().trim();
    if (!q) return list;
    return list.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        s.path.toLowerCase().includes(q) ||
        branchLabel(s.branch).toLowerCase().includes(q)
    );
  });

  const toggleSection = (key: string) => {
    setSectionsCollapsed((prev) => ({
      ...prev,
      [key]: !prev[key],
    }));
  };

  const isSectionCollapsed = (key: string) => {
    return Boolean(sectionsCollapsed()[key]);
  };

  return (
    <div class="sidebar-container">
      <Show
        when={props.root}
        fallback={
          <div class="sidebar-open-section">
            <div class="sidebar-open-card">
              <label for="open-path-input" class="sidebar-open-label">Open repository</label>
              <input
                id="open-path-input"
                type="text"
                class="sidebar-input"
                placeholder="/path/to/repository"
                value={props.pathInput}
                onInput={(e) => props.onPathInput(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") props.onOpenRepo(props.pathInput);
                }}
                ref={props.setPathInputEl}
              />
              <div class="open-repo-actions">
                <button type="button" class="sidebar-btn-primary" onClick={() => props.onOpenRepo(props.pathInput)}>
                  Open
                </button>
                <button
                  type="button"
                  class="sidebar-btn-secondary"
                  onClick={props.onBrowse}
                  title="Browse local folder"
                  aria-label="Browse local folder"
                >
                  <Icon name="folder-open" size={14} />
                  <span>Browse…</span>
                </button>
              </div>
              <Show when={props.openError}>
                <div class="text-danger sidebar-error">{props.openError}</div>
              </Show>
              <Show when={props.offerInitAt}>
                {(path) => (
                  <div class="init-offer">
                    <p class="text-muted">No repository found at {path()}.</p>
                    <button class="sidebar-btn-primary" onClick={() => props.onConfirmInit(path())}>
                      Initialize repository
                    </button>
                    <button class="collapse-toggle" onClick={props.onCancelInit}>
                      Cancel
                    </button>
                  </div>
                )}
              </Show>
            </div>

            <Show when={props.bookmarks.length > 0}>
              <div class="sidebar-bookmarks-wrapper">
                <div class="sidebar-section-header">
                  <span class="sidebar-section-title">BOOKMARKS</span>
                </div>
                <ul class="sidebar-bookmark-list">
                  <For each={props.bookmarks}>
                    {(b) => (
                      <li>
                        <button
                          type="button"
                          class="sidebar-bookmark-item"
                          onClick={() => props.onOpenRepo(b.root)}
                          title={b.root}
                        >
                          <Icon name="folder" size={13} />
                          <span class="bookmark-name">{b.root.split("/").filter(Boolean).pop() || b.root}</span>
                        </button>
                      </li>
                    )}
                  </For>
                </ul>
              </div>
            </Show>
          </div>
        }
      >
        <div class="sidebar-repo-explorer">
          <div class="sidebar-repo-header">
            <div class="sidebar-repo-title-row">
              <div class="sidebar-repo-title" title={props.root!}>
                <Icon name="folder" size={15} />
                <span class="sidebar-repo-name">{repoName()}</span>
              </div>
              <button
                type="button"
                class={`icon-btn-star ${props.isBookmarked ? "active" : ""}`}
                onClick={props.onToggleBookmark}
                title={props.isBookmarked ? "Remove bookmark" : "Add bookmark"}
                aria-label="Toggle bookmark"
              >
                <Icon name="star" size={14} />
              </button>
            </div>
            <div class="sidebar-branch-row" title={`Branch: ${branchName()}`}>
              <Icon name="branch" size={13} />
              <span class="sidebar-branch-name">{branchName()}</span>
              <Show when={aheadBehind()}>
                {(ab) => (
                  <span class="sidebar-branch-ab">
                    <Show when={ab().behind > 0}>
                      <span class="sidebar-ab-behind" title={`${ab().behind} commits behind upstream`}>
                        <Icon name="arrow-down" size={10} />
                        {ab().behind}
                      </span>
                    </Show>
                    <Show when={ab().ahead > 0}>
                      <span class="sidebar-ab-ahead" title={`${ab().ahead} commits ahead of upstream`}>
                        <Icon name="arrow-up" size={10} />
                        {ab().ahead}
                      </span>
                    </Show>
                  </span>
                )}
              </Show>
            </div>
          </div>

          <Show when={(props.submodules?.length ?? 0) > 8 || (props.branches?.length ?? 0) > 8}>
            <div class="sidebar-search-row">
              <Icon name="search" size={12} class="sidebar-search-icon" />
              <input
                type="text"
                class="sidebar-filter-input"
                placeholder="Filter branches & submodules…"
                value={searchFilter()}
                onInput={(e) => setSearchFilter(e.currentTarget.value)}
              />
              <Show when={searchFilter()}>
                <button
                  type="button"
                  class="sidebar-filter-clear"
                  onClick={() => setSearchFilter("")}
                  title="Clear filter"
                  aria-label="Clear filter"
                >
                  <Icon name="close" size={10} />
                </button>
              </Show>
            </div>
          </Show>

          <div class="sidebar-nav-section">
            {/* WORKSPACE SECTION */}
            <div class="sidebar-tree-group">
              <div
                class="sidebar-section-header sidebar-section-interactive"
                onClick={() => toggleSection("workspace")}
                role="button"
                tabIndex={0}
                aria-expanded={!isSectionCollapsed("workspace")}
              >
                <span class="sidebar-section-title-wrapper">
                  <Icon
                    name={isSectionCollapsed("workspace") ? "chevron-right" : "chevron-down"}
                    size={11}
                    class="sidebar-chevron"
                  />
                  <span class="sidebar-section-title">WORKSPACE</span>
                </span>
              </div>
              <Show when={!isSectionCollapsed("workspace")}>
                <ul class="sidebar-nav-list">
                  <li>
                    <button
                      type="button"
                      class={`sidebar-nav-item ${props.activeView === "working-copy" ? "active" : ""}`}
                      onClick={() => props.onSelectView("working-copy")}
                    >
                      <span class="sidebar-nav-item-left">
                        <Icon name="edit" size={14} />
                        <span>Working copy</span>
                      </span>
                      <Show when={workingCopyCount() > 0}>
                        <span class="sidebar-nav-badge count-active">{workingCopyCount()}</span>
                      </Show>
                    </button>
                  </li>
                  <li>
                    <button
                      type="button"
                      class={`sidebar-nav-item ${props.activeView === "history" ? "active" : ""}`}
                      onClick={() => props.onSelectView("history")}
                    >
                      <span class="sidebar-nav-item-left">
                        <Icon name="history" size={14} />
                        <span>History</span>
                      </span>
                    </button>
                  </li>
                  <li>
                    <button
                      type="button"
                      class={`sidebar-nav-item ${props.activeView === "reflog" ? "active" : ""}`}
                      onClick={() => props.onSelectView("reflog")}
                    >
                      <span class="sidebar-nav-item-left">
                        <Icon name="terminal" size={14} />
                        <span>Reflog</span>
                      </span>
                    </button>
                  </li>
                </ul>
              </Show>
            </div>

            {/* BRANCHES SECTION */}
            <div class="sidebar-tree-group">
              <div
                class="sidebar-section-header sidebar-section-interactive"
                onClick={() => toggleSection("branches")}
                role="button"
                tabIndex={0}
                aria-expanded={!isSectionCollapsed("branches")}
              >
                <span class="sidebar-section-title-wrapper">
                  <Icon
                    name={isSectionCollapsed("branches") ? "chevron-right" : "chevron-down"}
                    size={11}
                    class="sidebar-chevron"
                  />
                  <span class="sidebar-section-title">BRANCHES</span>
                </span>
                <span class="sidebar-section-actions">
                  <Show when={localBranches().length > 0}>
                    <span class="sidebar-section-count">{localBranches().length}</span>
                  </Show>
                  <button
                    type="button"
                    class="icon-btn-ghost-small"
                    onClick={(e) => {
                      e.stopPropagation();
                      props.onSelectView("branches");
                    }}
                    title="Open full Branches view"
                    aria-label="Open Branches view"
                  >
                    <Icon name="list" size={11} />
                  </button>
                </span>
              </div>
              <Show when={!isSectionCollapsed("branches")}>
                <ul class="sidebar-tree-list">
                  <For
                    each={localBranches()}
                    fallback={
                      <li class="sidebar-tree-empty">
                        <Show when={props.branches && props.branches.length > 0} fallback={<span>No branches</span>}>
                          <button
                            type="button"
                            class="sidebar-tree-more-btn"
                            onClick={() => props.onSelectView("branches")}
                          >
                            Open full Branches view…
                          </button>
                        </Show>
                      </li>
                    }
                  >
                    {(b) => {
                      const isHead = () => b.is_head;
                      return (
                        <li>
                          <button
                            type="button"
                            class={`sidebar-tree-item ${isHead() ? "tree-item-head" : ""}`}
                            onClick={() => {
                              if (isHead()) {
                                props.onSelectView("working-copy");
                              }
                            }}
                            onDblClick={() => {
                              if (!isHead() && props.onCheckoutBranch) {
                                props.onCheckoutBranch(b.name);
                              }
                            }}
                            title={
                              isHead()
                                ? `Current branch: ${b.name}`
                                : `Double-click to checkout '${b.name}'`
                            }
                          >
                            <span class="tree-item-left">
                              <Icon
                                name={isHead() ? "check" : "branch"}
                                size={12}
                                class={isHead() ? "icon-head" : "icon-branch"}
                              />
                              <span class="tree-item-label">{b.name}</span>
                            </span>
                            <Show when={b.ahead_behind && (b.ahead_behind[0] > 0 || b.ahead_behind[1] > 0)}>
                              <span class="tree-item-ab">
                                <Show when={b.ahead_behind![1] > 0}>
                                  <span class="ab-behind">
                                    <Icon name="arrow-down" size={9} />
                                    {b.ahead_behind![1]}
                                  </span>
                                </Show>
                                <Show when={b.ahead_behind![0] > 0}>
                                  <span class="ab-ahead">
                                    <Icon name="arrow-up" size={9} />
                                    {b.ahead_behind![0]}
                                  </span>
                                </Show>
                              </span>
                            </Show>
                          </button>
                        </li>
                      );
                    }}
                  </For>
                  <Show when={props.branches && props.branches.length > localBranches().length}>
                    <li>
                      <button
                        type="button"
                        class="sidebar-tree-more-btn"
                        onClick={() => props.onSelectView("branches")}
                      >
                        View all {props.branches!.length} branches (remotes)…
                      </button>
                    </li>
                  </Show>
                </ul>
              </Show>
            </div>

            {/* SUBMODULES SECTION */}
            <div class="sidebar-tree-group">
              <div
                class="sidebar-section-header sidebar-section-interactive"
                onClick={() => toggleSection("submodules")}
                role="button"
                tabIndex={0}
                aria-expanded={!isSectionCollapsed("submodules")}
              >
                <span class="sidebar-section-title-wrapper">
                  <Icon
                    name={isSectionCollapsed("submodules") ? "chevron-right" : "chevron-down"}
                    size={11}
                    class="sidebar-chevron"
                  />
                  <span class="sidebar-section-title">SUBMODULES</span>
                </span>
                <span class="sidebar-section-actions">
                  <Show when={props.submoduleCount > 0}>
                    <span class="sidebar-section-count">{props.submoduleCount}</span>
                  </Show>
                  <button
                    type="button"
                    class="icon-btn-ghost-small"
                    onClick={(e) => {
                      e.stopPropagation();
                      props.onSelectView("submodules");
                    }}
                    title="Open Submodule Matrix"
                    aria-label="Open Submodule Matrix"
                  >
                    <Icon name="package" size={11} />
                  </button>
                </span>
              </div>
              <Show when={!isSectionCollapsed("submodules")}>
                <ul class="sidebar-tree-list">
                  <For
                    each={filteredSubmodules()}
                    fallback={
                      <li class="sidebar-tree-empty">
                        <Show
                          when={props.submoduleCount > 0}
                          fallback={<span>No submodules in repository</span>}
                        >
                          <button
                            type="button"
                            class="sidebar-tree-more-btn"
                            onClick={() => props.onSelectView("submodules")}
                          >
                            Open Submodule Matrix ({props.submoduleCount})…
                          </button>
                        </Show>
                      </li>
                    }
                  >
                    {(s) => {
                      const dirty = () => isDirty(s);
                      const detached = () => isDetached(s);
                      const div = () => gitlinkAheadBehind(s);
                      const subBranch = () => branchLabel(s.branch);

                      return (
                        <li>
                          <button
                            type="button"
                            class="sidebar-tree-item submodule-item"
                            onClick={() => {
                              if (props.onDrillInSubmodule && s.initialized) {
                                props.onDrillInSubmodule(s.path, s.name);
                              } else {
                                props.onSelectView("submodules");
                              }
                            }}
                            title={
                              s.initialized
                                ? `Click to open submodule ${s.name} tab (${subBranch()})`
                                : `Submodule ${s.name} (uninitialized)`
                            }
                          >
                            <span class="tree-item-left">
                              <span class="submodule-status-dot-wrapper">
                                <Show
                                  when={dirty()}
                                  fallback={
                                    <Show
                                      when={div()}
                                      fallback={
                                        <Show
                                          when={s.initialized}
                                          fallback={<Icon name="warning" size={11} class="sub-dot-uninit" />}
                                        >
                                          <Icon name="check" size={11} class="sub-dot-sync" />
                                        </Show>
                                      }
                                    >
                                      {(d) => (
                                        <Show
                                          when={d().ahead > 0 && d().behind > 0}
                                          fallback={
                                            d().ahead > 0 ? (
                                              <Icon name="arrow-up" size={11} class="sub-dot-ahead" />
                                            ) : (
                                              <Icon name="arrow-down" size={11} class="sub-dot-behind" />
                                            )
                                          }
                                        >
                                          <Icon name="arrow-up-down" size={11} class="sub-dot-diverged" />
                                        </Show>
                                      )}
                                    </Show>
                                  }
                                >
                                  <Icon name="dirty" size={8} class="sub-dot-dirty" />
                                </Show>
                              </span>
                              <span class="tree-item-label sub-name">{s.name}</span>
                            </span>
                            <span class="sub-branch-pill" classList={{ "sub-branch-detached": detached() }}>
                              {detached() ? "detached" : subBranch()}
                            </span>
                          </button>
                        </li>
                      );
                    }}
                  </For>
                  <Show when={props.submoduleCount > 0}>
                    <li>
                      <button
                        type="button"
                        class="sidebar-tree-more-btn"
                        onClick={() => props.onSelectView("submodules")}
                      >
                        Open Submodule Matrix ({props.submoduleCount})…
                      </button>
                    </li>
                  </Show>
                </ul>
              </Show>
            </div>

            {/* TAGS & STASHES SECTION */}
            <div class="sidebar-tree-group">
              <div
                class="sidebar-section-header sidebar-section-interactive"
                onClick={() => toggleSection("tags")}
                role="button"
                tabIndex={0}
                aria-expanded={!isSectionCollapsed("tags")}
              >
                <span class="sidebar-section-title-wrapper">
                  <Icon
                    name={isSectionCollapsed("tags") ? "chevron-right" : "chevron-down"}
                    size={11}
                    class="sidebar-chevron"
                  />
                  <span class="sidebar-section-title">TAGS & STASHES</span>
                </span>
              </div>
              <Show when={!isSectionCollapsed("tags")}>
                <ul class="sidebar-nav-list">
                  <li>
                    <button
                      type="button"
                      class={`sidebar-nav-item ${props.activeView === "tags" ? "active" : ""}`}
                      onClick={() => props.onSelectView("tags")}
                    >
                      <span class="sidebar-nav-item-left">
                        <Icon name="tag" size={14} />
                        <span>Tags</span>
                      </span>
                    </button>
                  </li>
                  <li>
                    <button
                      type="button"
                      class={`sidebar-nav-item ${props.activeView === "stashes" ? "active" : ""}`}
                      onClick={() => props.onSelectView("stashes")}
                    >
                      <span class="sidebar-nav-item-left">
                        <Icon name="archive" size={14} />
                        <span>Stashes</span>
                      </span>
                      <Show when={stashCount() > 0}>
                        <span class="sidebar-nav-badge">{stashCount()}</span>
                      </Show>
                    </button>
                  </li>
                </ul>
              </Show>
            </div>
          </div>

          <div class="sidebar-bottom-section">
            <div class="sidebar-section-header">
              <span class="sidebar-section-title">BOOKMARKS</span>
              <button
                type="button"
                class="icon-btn-ghost"
                onClick={props.onBrowse}
                title="Open another repository (Browse...)"
                aria-label="Open another repository"
              >
                <Icon name="plus" size={13} />
              </button>
            </div>
            <ul class="sidebar-bookmark-list">
              <For each={props.bookmarks}>
                {(b) => {
                  const isCurrent = () => b.root === props.root;
                  return (
                    <li>
                      <button
                        type="button"
                        class={`sidebar-bookmark-item ${isCurrent() ? "current" : ""}`}
                        onClick={() => props.onOpenRepo(b.root)}
                        title={b.root}
                      >
                        <Icon name="folder" size={13} />
                        <span class="bookmark-name">{b.root.split("/").filter(Boolean).pop() || b.root}</span>
                      </button>
                    </li>
                  );
                }}
              </For>
            </ul>
          </div>
        </div>
      </Show>
    </div>
  );
};
