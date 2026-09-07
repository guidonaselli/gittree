import { type Component, For, Show, createResource, createSignal } from "solid-js";
import { getBlame } from "../../api/commands";
import { CommitDetailView } from "../history/CommitDetailView";
import { groupBlameLines } from "./blame-grouping";

function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString();
}

export const BlameView: Component<{ root: string; path: string }> = (props) => {
  const [ignoreWhitespace, setIgnoreWhitespace] = createSignal(false);
  const [revStack, setRevStack] = createSignal<string[]>([]);
  const [selectedSha, setSelectedSha] = createSignal<string | null>(null);

  const currentRev = () => revStack()[revStack().length - 1];

  const [lines] = createResource(
    () => [props.root, props.path, ignoreWhitespace(), currentRev()] as const,
    ([root, path, ws, rev]) => getBlame(root, path, ws, rev),
  );

  const blocks = () => groupBlameLines(lines() ?? []);

  return (
    <div class="blame-view">
      <div class="blame-toolbar">
        <label class="diff-toolbar-option">
          <input type="checkbox" checked={ignoreWhitespace()} onChange={(e) => setIgnoreWhitespace(e.currentTarget.checked)} />
          Ignore whitespace
        </label>
        <Show when={revStack().length > 0}>
          <span class="text-muted">blaming as of {revStack()[revStack().length - 1]}</span>
          <button class="working-copy-action" onClick={() => setRevStack(revStack().slice(0, -1))}>
            Back
          </button>
          <button class="working-copy-action" onClick={() => setRevStack([])}>
            Reset to latest
          </button>
        </Show>
      </div>
      <Show when={lines.error}>
        <p class="diff-apply-error">{String(lines.error)}</p>
      </Show>
      <Show when={lines.loading}>
        <p class="text-muted">Loading blame…</p>
      </Show>
      <div class="blame-lines">
        <For each={blocks()}>
          {(block) => (
            <div class="blame-block">
              <div class="blame-meta">
                <button class="blame-sha" onClick={() => setSelectedSha(block.sha)}>
                  {block.sha.slice(0, 7)}
                </button>
                <span class="blame-author">{block.author_name}</span>
                <span class="blame-date">{formatDate(block.author_time)}</span>
                <span class="blame-summary">{block.summary}</span>
                <Show when={!block.is_boundary}>
                  <button class="blame-reblame" onClick={() => setRevStack([...revStack(), `${block.sha}^`])}>
                    Blame before this
                  </button>
                </Show>
              </div>
              <div class="blame-block-lines">
                <For each={block.lines}>
                  {(l) => (
                    <div class="blame-line">
                      <span class="blame-line-no">{l.final_line_no}</span>
                      <span class="blame-line-content">{l.content}</span>
                    </div>
                  )}
                </For>
              </div>
            </div>
          )}
        </For>
      </div>
      <Show when={selectedSha()}>{(sha) => <CommitDetailView root={props.root} sha={sha()} />}</Show>
    </div>
  );
};
