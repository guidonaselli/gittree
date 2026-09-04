import { type Component, For, Show, createResource, createSignal } from "solid-js";
import {
  getFileDiff,
  stageFileHunks,
  stageFileLines,
  unstageFileHunks,
  unstageFileLines,
} from "../../api/commands";
import type { Hunk } from "../../api/types";

function isChangedLine(line: string): boolean {
  return line.startsWith("+") || line.startsWith("-");
}

function lineClass(line: string, selected: boolean): string {
  const base = line.startsWith("+") ? "diff-line diff-line-add" : line.startsWith("-") ? "diff-line diff-line-remove" : "diff-line";
  return selected ? `${base} diff-line-selected` : base;
}

function HunkBlock(props: {
  hunk: Hunk;
  hunkIndex: number;
  staged: boolean;
  selected: Set<number>;
  onToggleLine: (hunkIndex: number, lineIndex: number) => void;
  onAction: () => void;
  onActionSelected: () => void;
}) {
  const selectedInThisHunk = () =>
    [...props.selected].filter((k) => Math.floor(k / 100000) === props.hunkIndex).length > 0;

  return (
    <div class="diff-hunk">
      <div class="diff-hunk-header">
        <span>{props.hunk.header}</span>
        <div class="diff-hunk-actions">
          <Show when={selectedInThisHunk()}>
            <button class="working-copy-action" onClick={props.onActionSelected}>
              {props.staged ? "Unstage selected" : "Stage selected"}
            </button>
          </Show>
          <button class="working-copy-action" onClick={props.onAction}>
            {props.staged ? "Unstage hunk" : "Stage hunk"}
          </button>
        </div>
      </div>
      <For each={props.hunk.lines}>
        {(line, i) => {
          const key = props.hunkIndex * 100000 + i();
          const changed = isChangedLine(line);
          const selected = () => props.selected.has(key);
          if (!changed) {
            return <div class={lineClass(line, false)}>{line}</div>;
          }
          return (
            <button
              type="button"
              class={`${lineClass(line, false)} diff-line-selectable`}
              classList={{ "diff-line-selected": selected() }}
              onClick={() => props.onToggleLine(props.hunkIndex, i())}
            >
              {line}
            </button>
          );
        }}
      </For>
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
  const [selected, setSelected] = createSignal<Set<number>>(new Set());

  function toggleLine(hunkIndex: number, lineIndex: number) {
    const key = hunkIndex * 100000 + lineIndex;
    const next = new Set<number>(selected());
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setSelected(next);
  }

  async function afterChange() {
    setSelected(new Set<number>());
    await refetch();
    props.onChanged();
  }

  async function stageHunk(index: number) {
    try {
      await stageFileHunks(props.root, props.path, [index]);
    } finally {
      await afterChange();
    }
  }

  async function unstageHunk(index: number) {
    try {
      await unstageFileHunks(props.root, props.path, [index]);
    } finally {
      await afterChange();
    }
  }

  async function stageSelected(hunkIndex: number) {
    const lines = [...selected()]
      .filter((k) => Math.floor(k / 100000) === hunkIndex)
      .map((k) => k % 100000);
    try {
      await stageFileLines(props.root, props.path, hunkIndex, lines);
    } finally {
      await afterChange();
    }
  }

  async function unstageSelected(hunkIndex: number) {
    const lines = [...selected()]
      .filter((k) => Math.floor(k / 100000) === hunkIndex)
      .map((k) => k % 100000);
    try {
      await unstageFileLines(props.root, props.path, hunkIndex, lines);
    } finally {
      await afterChange();
    }
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
              hunkIndex={i()}
              staged={props.staged}
              selected={selected()}
              onToggleLine={toggleLine}
              onAction={() => (props.staged ? unstageHunk(i()) : stageHunk(i()))}
              onActionSelected={() => (props.staged ? unstageSelected(i()) : stageSelected(i()))}
            />
          )}
        </For>
      </Show>
    </div>
  );
};
