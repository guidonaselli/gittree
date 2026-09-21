import { type Component, Show, createMemo } from "solid-js";
import { branchLabel, isKnown, unknownReason, upstreamBasisLabel, type RepositoryState } from "../../api/types";

export const RepositoryStatus: Component<{
  state: RepositoryState;
  onOpenBranches?: () => void;
}> = (props) => {
  const branch = createMemo(() => branchLabel(props.state.branch));
  const inProgress = createMemo(() => props.state.in_progress);
  const pathsKnown = createMemo(() => isKnown(props.state.paths));
  const paths = createMemo(() => (isKnown(props.state.paths) ? props.state.paths.value : null));
  const pathsUnknownReason = createMemo(() => unknownReason(props.state.paths));
  const stashCount = createMemo(() => (isKnown(props.state.stash_count) ? props.state.stash_count.value : 0));

  const aheadBehind = createMemo(() =>
    isKnown(props.state.ahead_behind) ? props.state.ahead_behind.value : null
  );
  const upstreamInfo = createMemo(() => {
    const ab = aheadBehind();
    return ab ? upstreamBasisLabel(ab.basis) : null;
  });

  return (
    <div class="repo-status">
      <div class="repo-status-branch">
        <span class="repo-branch-name">{branch()}</span>
        <Show when={upstreamInfo()}>
          {(info) => {
            const ab = aheadBehind();
            return (
              <Show
                when={info().inferred}
                fallback={
                  <span class="branch-badge-upstream">
                    {ab ? `↑${ab.ahead} ↓${ab.behind} vs ${info().label}` : info().label}
                  </span>
                }
              >
                <span
                  class="branch-badge-inferred"
                  title="Inferred basis: no explicit @{u} configured, resolved to matching default remote branch"
                >
                  {ab ? `↑${ab.ahead} ↓${ab.behind} vs ${info().label} (inferred)` : `${info().label} (inferred)`}
                </span>
              </Show>
            );
          }}
        </Show>
        <Show when={props.onOpenBranches}>
          <button class="collapse-toggle" onClick={props.onOpenBranches}>
            Branches
          </button>
        </Show>
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
