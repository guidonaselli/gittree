import { type Component, For, Show, createResource } from "solid-js";
import { getFileDiff, stageFileHunks, unstageFileHunks } from "../../api/commands";
import type { Hunk } from "../../api/types";

function lineClass(line: string): string {
  if (line.startsWith("+")) return "diff-line diff-line-add";
  if (line.startsWith("-")) return "diff-line diff-line-remove";
  return "diff-line";
}

function HunkBlock(props: { hunk: Hunk; actionLabel: string; onAction: () => void }) {
  return (
    <div class="diff-hunk">
      <div class="diff-hunk-header">
        <span>{props.hunk.header}</span>
        <button class="working-copy-action" onClick={props.onAction}>
          {props.actionLabel}
        </button>
      </div>
      <For each={props.hunk.lines}>{(line) => <div class={lineClass(line)}>{line}</div>}</For>
    </div>
  );
}

export const HunkDiffView: Component<{
  root: string;
  path: string;
  staged: boolean;
  onChanged: () => void;
}> = (props) => {
  const [diff, { refetch }] = createResource(
    () => [props.root, props.path, props.staged] as const,
    ([root, path, staged]) => getFileDiff(root, path, staged),
  );

  async function stageOne(index: number) {
    await stageFileHunks(props.root, props.path, [index]);
    await refetch();
    props.onChanged();
  }

  async function unstageOne(index: number) {
    await unstageFileHunks(props.root, props.path, [index]);
    await refetch();
    props.onChanged();
  }

  return (
    <div class="diff-view">
      <Show when={diff.loading}>
        <p class="text-muted">Loading diff…</p>
      </Show>
      <Show when={diff() && diff()!.is_binary}>
        <p class="text-muted">Binary file, no hunk view.</p>
      </Show>
      <Show when={diff() && !diff()!.is_binary}>
        <For each={diff()!.hunks}>
          {(hunk, i) => (
            <HunkBlock
              hunk={hunk}
              actionLabel={props.staged ? "Unstage hunk" : "Stage hunk"}
              onAction={() => (props.staged ? unstageOne(i()) : stageOne(i()))}
            />
          )}
        </For>
      </Show>
    </div>
  );
};
