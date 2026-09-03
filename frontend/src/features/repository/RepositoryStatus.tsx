import { type Component, Show, createMemo } from "solid-js";
import { branchLabel, isKnown, unknownReason, type RepositoryState } from "../../api/types";

export const RepositoryStatus: Component<{ state: RepositoryState }> = (props) => {
  const branch = createMemo(() => branchLabel(props.state.branch));
  const inProgress = createMemo(() => props.state.in_progress);
  const pathsKnown = createMemo(() => isKnown(props.state.paths));
  const paths = createMemo(() => (isKnown(props.state.paths) ? props.state.paths.value : null));
  const pathsUnknownReason = createMemo(() => unknownReason(props.state.paths));
  const stashCount = createMemo(() => (isKnown(props.state.stash_count) ? props.state.stash_count.value : 0));

  return (
    <div class="repo-status">
      <div class="repo-status-branch">
        {branch()}
        <Show when={inProgress() !== "None"}>
          <span class="badge badge-warning">{inProgress()}</span>
        </Show>
      </div>
      <Show
        when={pathsKnown()}
        fallback={<div class="text-muted">status unknown ({pathsUnknownReason()})</div>}
      >
        <div class="repo-status-counts">
          <span>{paths()!.staged} staged</span>
          <span>{paths()!.unstaged} unstaged</span>
          <span>{paths()!.untracked} untracked</span>
          <Show when={paths()!.conflicted > 0}>
            <span class="text-danger">{paths()!.conflicted} conflicted</span>
          </Show>
        </div>
      </Show>
      <Show when={stashCount() > 0}>
        <div class="text-muted">{stashCount()} stash(es)</div>
      </Show>
    </div>
  );
};
