import type { Component } from "solid-js";
import { Show } from "solid-js";
import type { SubmoduleConflictInfo } from "../../api/types";
import "./conflicts.css";

export interface SubmoduleConflictResolverProps {
  path: string;
  submoduleInfo: SubmoduleConflictInfo;
  onResolveOurs: () => void;
  onResolveTheirs: () => void;
  onResolveCommit: (sha: string) => void;
  loading?: boolean;
}

export const SubmoduleConflictResolver: Component<SubmoduleConflictResolverProps> = (props) => {
  return (
    <div class="conflict-submodule-details">
      <div class="candidate-header">
        <span class="candidate-label">Submodule Gitlink Candidates for {props.path}</span>
      </div>
      <p style={{ "font-size": "var(--font-size-sm)", color: "var(--color-text-muted)", margin: "var(--space-1) 0" }}>
        Both branches modified this submodule gitlink independently. Choose which commit to accept:
      </p>

      <div class="submodule-grid">
        {/* Ours candidate */}
        <div class="candidate-commit-card">
          <div class="candidate-header">
            <span class="candidate-label">Ours (Current Branch)</span>
            <Show when={props.submoduleInfo.ours_commit}>
              {(commit) => <span class="candidate-sha">{commit().short_sha}</span>}
            </Show>
          </div>
          <Show
            when={props.submoduleInfo.ours_commit}
            fallback={<div class="candidate-subject">(No commit data available)</div>}
          >
            {(commit) => (
              <>
                <div class="candidate-subject">{commit().subject}</div>
                <div class="candidate-meta">
                  {commit().author} {commit().date ? `• ${commit().date}` : ""}
                </div>
              </>
            )}
          </Show>
          <button
            class="conflict-btn conflict-btn-primary"
            disabled={props.loading || !props.submoduleInfo.ours_commit}
            onClick={() => {
              if (props.submoduleInfo.ours_commit) {
                props.onResolveCommit(props.submoduleInfo.ours_commit.sha);
              } else {
                props.onResolveOurs();
              }
            }}
          >
            Accept Ours {props.submoduleInfo.ours_commit ? `(${props.submoduleInfo.ours_commit.short_sha})` : ""}
          </button>
        </div>

        {/* Theirs candidate */}
        <div class="candidate-commit-card">
          <div class="candidate-header">
            <span class="candidate-label">Theirs (Incoming Branch)</span>
            <Show when={props.submoduleInfo.theirs_commit}>
              {(commit) => <span class="candidate-sha">{commit().short_sha}</span>}
            </Show>
          </div>
          <Show
            when={props.submoduleInfo.theirs_commit}
            fallback={<div class="candidate-subject">(No commit data available)</div>}
          >
            {(commit) => (
              <>
                <div class="candidate-subject">{commit().subject}</div>
                <div class="candidate-meta">
                  {commit().author} {commit().date ? `• ${commit().date}` : ""}
                </div>
              </>
            )}
          </Show>
          <button
            class="conflict-btn conflict-btn-primary"
            disabled={props.loading || !props.submoduleInfo.theirs_commit}
            onClick={() => {
              if (props.submoduleInfo.theirs_commit) {
                props.onResolveCommit(props.submoduleInfo.theirs_commit.sha);
              } else {
                props.onResolveTheirs();
              }
            }}
          >
            Accept Theirs {props.submoduleInfo.theirs_commit ? `(${props.submoduleInfo.theirs_commit.short_sha})` : ""}
          </button>
        </div>
      </div>

      <Show when={props.submoduleInfo.base_commit}>
        {(base) => (
          <div style={{ "margin-top": "var(--space-2)", "font-size": "var(--font-size-xs)", color: "var(--color-text-muted)" }}>
            Common ancestor commit: <code>{base().short_sha}</code> — {base().subject}
          </div>
        )}
      </Show>
    </div>
  );
};
