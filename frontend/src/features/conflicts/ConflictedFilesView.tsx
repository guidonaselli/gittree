import { createSignal, onMount, type Component, For, Show } from "solid-js";
import type { ConflictItem, MergetoolConfig } from "../../api/types";
import { getMergetoolConfig, launchMergetool, resolveConflict } from "../../api/commands";
import { SubmoduleConflictResolver } from "./SubmoduleConflictResolver";
import "./conflicts.css";
import { Icon } from "../../ui/Icon";

export interface ConflictedFilesViewProps {
  root: string;
  conflicts: ConflictItem[];
  onRefresh: () => void;
  onStageFile: (path: string) => void;
}

export const ConflictedFilesView: Component<ConflictedFilesViewProps> = (props) => {
  const [loadingPath, setLoadingPath] = createSignal<string | null>(null);
  const [actionError, setActionError] = createSignal<string | null>(null);
  const [mergetoolConfig, setMergetoolConfig] = createSignal<MergetoolConfig | null>(null);
  const [collapsedSubmodules, setCollapsedSubmodules] = createSignal<Record<string, boolean>>({});

  onMount(async () => {
    try {
      const cfg = await getMergetoolConfig(props.root);
      setMergetoolConfig(cfg);
    } catch {
      // Mergetool config is optional
    }
  });

  async function handleResolveOurs(path: string) {
    setLoadingPath(path);
    setActionError(null);
    try {
      await resolveConflict(props.root, path, { resolution_type: "ours" });
      props.onRefresh();
    } catch (err) {
      setActionError(String(err));
    } finally {
      setLoadingPath(null);
    }
  }

  async function handleResolveTheirs(path: string) {
    setLoadingPath(path);
    setActionError(null);
    try {
      await resolveConflict(props.root, path, { resolution_type: "theirs" });
      props.onRefresh();
    } catch (err) {
      setActionError(String(err));
    } finally {
      setLoadingPath(null);
    }
  }

  async function handleResolveSubmoduleCommit(path: string, sha: string) {
    setLoadingPath(path);
    setActionError(null);
    try {
      await resolveConflict(props.root, path, {
        resolution_type: "submodule_commit",
        sha,
      });
      props.onRefresh();
    } catch (err) {
      setActionError(String(err));
    } finally {
      setLoadingPath(null);
    }
  }

  async function handleLaunchMergetool(path: string) {
    setLoadingPath(path);
    setActionError(null);
    try {
      const outcome = await launchMergetool(props.root, path);
      if (!outcome.success) {
        setActionError(outcome.stderr || outcome.stdout || "Mergetool exited with non-zero status");
      } else {
        props.onRefresh();
      }
    } catch (err) {
      setActionError(String(err));
    } finally {
      setLoadingPath(null);
    }
  }

  function getBadgeClass(type: string): string {
    switch (type) {
      case "both_modified":
        return "badge-both-modified";
      case "both_added":
        return "badge-both-added";
      case "delete_modify":
        return "badge-delete-modify";
      case "rename_rename":
        return "badge-rename";
      case "submodule":
        return "badge-submodule";
      default:
        return "badge-both-modified";
    }
  }

  function getBadgeLabel(item: ConflictItem): string {
    switch (item.conflict_type) {
      case "both_modified":
        return `Both Modified (${item.conflict_code})`;
      case "both_added":
        return `Both Added (${item.conflict_code})`;
      case "delete_modify":
        return `Delete / Modify (${item.conflict_code})`;
      case "rename_rename":
        return `Rename Conflict (${item.conflict_code})`;
      case "submodule":
        return "Submodule Gitlink";
      default:
        return `Conflict (${item.conflict_code})`;
    }
  }

  return (
    <section class="conflicts-section">
      <div class="conflicts-header">
        <h3 class="conflicts-title">
          <Icon name="warning" size={16} />
          <span>Conflicted Files ({props.conflicts.length})</span>
        </h3>
        <span style={{ "font-size": "var(--font-size-xs)", color: "var(--color-text-muted)" }}>
          Resolve conflicts by choosing Ours, Theirs, or running a Mergetool.
        </span>
      </div>

      <Show when={actionError()}>
        {(err) => (
          <div class="conflict-guard-warning">
            <strong>Error:</strong> {err()}
          </div>
        )}
      </Show>

      <div class="conflicts-list">
        <For each={props.conflicts}>
          {(item) => {
            const isBusy = () => loadingPath() === item.path;
            const isSubmodule = () => item.is_submodule;
            const isCollapsed = () => !!collapsedSubmodules()[item.path];

            return (
              <div class="conflict-card">
                <div class="conflict-card-row">
                  <div class="conflict-info">
                    <span class={`conflict-type-badge ${getBadgeClass(item.conflict_type)}`}>
                      {getBadgeLabel(item)}
                    </span>
                    <span class="conflict-path">{item.path}</span>
                    <Show when={item.marker_info?.has_markers}>
                      <span class="conflict-marker-warning">
                        <Icon name="warning" size={12} />
                        <span>Markers on line{item.marker_info!.marker_lines.length > 1 ? "s" : ""}: {item.marker_info!.marker_lines.join(", ")}</span>
                      </span>
                    </Show>
                  </div>

                  <div class="conflict-actions">
                    <Show
                      when={isSubmodule()}
                      fallback={
                        <>
                          <button
                            class="conflict-btn"
                            disabled={isBusy()}
                            onClick={() => handleResolveOurs(item.path)}
                            title="Accept our version and stage"
                          >
                            Resolve Ours
                          </button>
                          <button
                            class="conflict-btn"
                            disabled={isBusy()}
                            onClick={() => handleResolveTheirs(item.path)}
                            title="Accept incoming version and stage"
                          >
                            Resolve Theirs
                          </button>
                          <button
                            class="conflict-btn"
                            disabled={isBusy()}
                            onClick={() => handleLaunchMergetool(item.path)}
                            title={
                              mergetoolConfig()?.configured_tool
                                ? `Launch configured mergetool (${mergetoolConfig()!.configured_tool})`
                                : "Launch mergetool"
                            }
                          >
                            Mergetool
                          </button>
                          <button
                            class="conflict-btn conflict-btn-primary"
                            disabled={isBusy()}
                            onClick={() => props.onStageFile(item.path)}
                            title="Stage file (protected by conflict marker guard)"
                          >
                            Stage
                          </button>
                        </>
                      }
                    >
                      <button
                        class="conflict-btn"
                        onClick={() =>
                          setCollapsedSubmodules((curr) => ({
                            ...curr,
                            [item.path]: !curr[item.path],
                          }))
                        }
                      >
                        {isCollapsed() ? "Show Candidates" : "Hide Candidates"}
                      </button>
                    </Show>
                  </div>
                </div>

                {/* Submodule conflict details - expanded by default */}
                <Show when={isSubmodule() && !isCollapsed()}>
                  <Show when={item.submodule_info}>
                    {(info) => (
                      <SubmoduleConflictResolver
                        path={item.path}
                        submoduleInfo={info()}
                        onResolveOurs={() => handleResolveOurs(item.path)}
                        onResolveTheirs={() => handleResolveTheirs(item.path)}
                        onResolveCommit={(sha) => handleResolveSubmoduleCommit(item.path, sha)}
                        loading={isBusy()}
                      />
                    )}
                  </Show>
                </Show>
              </div>
            );
          }}
        </For>
      </div>
    </section>
  );
};
