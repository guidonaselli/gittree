import { type Component, For, Show, createMemo } from "solid-js";
import {
  branchLabel,
  isKnown,
  type Bookmark,
  type RepositoryState,
  type Resolved,
  type WorkingCopyStatus,
} from "../../api/types";
import { Icon } from "../../ui/Icon";
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

          <div class="sidebar-nav-section">
            <div class="sidebar-section-header">
              <span class="sidebar-section-title">REPOSITORY</span>
            </div>
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
                  class={`sidebar-nav-item ${props.activeView === "submodules" ? "active" : ""}`}
                  onClick={() => props.onSelectView("submodules")}
                >
                  <span class="sidebar-nav-item-left">
                    <Icon name="package" size={14} />
                    <span>Submodules</span>
                  </span>
                  <Show when={props.submoduleCount > 0}>
                    <span class="sidebar-nav-badge">{props.submoduleCount}</span>
                  </Show>
                </button>
              </li>
              <li>
                <button
                  type="button"
                  class={`sidebar-nav-item ${props.activeView === "branches" ? "active" : ""}`}
                  onClick={() => props.onSelectView("branches")}
                >
                  <span class="sidebar-nav-item-left">
                    <Icon name="branch" size={14} />
                    <span>Branches</span>
                  </span>
                </button>
              </li>
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
