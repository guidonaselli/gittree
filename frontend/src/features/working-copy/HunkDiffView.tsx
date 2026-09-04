import { type Component, For, Show, createMemo, createResource, createSignal } from "solid-js";
import {
  getFileDiffWithOptions,
  stageFileHunks,
  stageFileLines,
  unstageFileHunks,
  unstageFileLines,
} from "../../api/commands";
import type { Hunk } from "../../api/types";
import { highlightLine, type Token } from "../../diff/highlight";
import { pairHunkLines, type PairedRow } from "../../diff/side-by-side";
import { wordDiff } from "../../diff/word-diff";
import { NonTextualDiffView } from "./NonTextualDiffView";

const FULL_FILE_CONTEXT = 1_000_000;
const LARGE_DIFF_LINE_THRESHOLD = 2000;

function isChangedLine(line: string): boolean {
  return line.startsWith("+") || line.startsWith("-");
}

function lineClass(line: string, selected: boolean): string {
  const base = line.startsWith("+") ? "diff-line diff-line-add" : line.startsWith("-") ? "diff-line diff-line-remove" : "diff-line";
  return selected ? `${base} diff-line-selected` : base;
}

/** Renders syntax-highlighted tokens for a whole (unpaired) line. */
function SyntaxSpans(props: { text: string }) {
  return (
    <For each={highlightLine(props.text)}>
      {(tok: Token) => (tok.class ? <span class={tok.class}>{tok.text}</span> : <>{tok.text}</>)}
    </For>
  );
}

/** Renders word-diff spans for one side of a paired change line. */
function WordDiffSpans(props: { spans: { text: string; changed: boolean }[] }) {
  return (
    <For each={props.spans}>
      {(span) => (span.changed ? <mark class="diff-word-changed">{span.text}</mark> : <>{span.text}</>)}
    </For>
  );
}

/** Finds runs of equal-length paired removal/addition blocks so they can get word-level highlighting. */
function pairedChangeIndices(lines: string[]): Map<number, number> {
  const pairs = new Map<number, number>();
  let i = 0;
  while (i < lines.length) {
    if (lines[i].startsWith("-")) {
      const removedStart = i;
      while (i < lines.length && lines[i].startsWith("-")) i++;
      const removedCount = i - removedStart;
      const addedStart = i;
      while (i < lines.length && lines[i].startsWith("+")) i++;
      const addedCount = i - addedStart;
      if (removedCount === addedCount) {
        for (let k = 0; k < removedCount; k++) {
          pairs.set(removedStart + k, addedStart + k);
          pairs.set(addedStart + k, removedStart + k);
        }
      }
    } else {
      i++;
    }
  }
  return pairs;
}

function InlineHunkBody(props: { hunk: Hunk; hunkIndex: number; selected: Set<number>; onToggleLine: (h: number, l: number) => void }) {
  const paired = createMemo(() => pairedChangeIndices(props.hunk.lines));
  return (
    <For each={props.hunk.lines}>
      {(line, i) => {
        const key = props.hunkIndex * 100000 + i();
        const changed = isChangedLine(line);
        const selected = () => props.selected.has(key);
        const partner = () => paired().get(i());
        const content = () => {
          const p = partner();
          if (changed && p !== undefined) {
            const otherLine = props.hunk.lines[p];
            const { oldSpans, newSpans } = line.startsWith("-")
              ? wordDiff(line.slice(1), otherLine.slice(1))
              : wordDiff(otherLine.slice(1), line.slice(1));
            return <WordDiffSpans spans={line.startsWith("-") ? oldSpans : newSpans} />;
          }
          return <SyntaxSpans text={line.slice(1)} />;
        };
        if (!changed) {
          return (
            <div class={lineClass(line, false)}>
              <SyntaxSpans text={line.startsWith(" ") ? line.slice(1) : line} />
            </div>
          );
        }
        return (
          <button
            type="button"
            class={`${lineClass(line, false)} diff-line-selectable`}
            classList={{ "diff-line-selected": selected() }}
            onClick={() => props.onToggleLine(props.hunkIndex, i())}
          >
            <span class="diff-line-prefix">{line[0]}</span>
            {content()}
          </button>
        );
      }}
    </For>
  );
}

function SideBySideHunkBody(props: { hunk: Hunk; hunkIndex: number; selected: Set<number>; onToggleLine: (h: number, l: number) => void }) {
  const rows = createMemo(() => pairHunkLines(props.hunk.lines));

  function cell(row: PairedRow, side: "left" | "right") {
    const index = side === "left" ? row.leftIndex : row.rightIndex;
    const text = side === "left" ? row.leftText : row.rightText;
    if (index === null || text === null) {
      return <div class="diff-sbs-cell diff-sbs-empty" />;
    }
    const key = props.hunkIndex * 100000 + index;
    const selected = () => props.selected.has(key);
    if (row.isContext) {
      return (
        <div class="diff-sbs-cell">
          <SyntaxSpans text={text} />
        </div>
      );
    }
    const hasBothSides = row.leftText !== null && row.rightText !== null;
    const wordSpans = () => {
      if (!hasBothSides) return null;
      const { oldSpans, newSpans } = wordDiff(row.leftText ?? "", row.rightText ?? "");
      return side === "left" ? oldSpans : newSpans;
    };
    return (
      <button
        type="button"
        class={`diff-sbs-cell diff-sbs-selectable ${side === "left" ? "diff-sbs-remove" : "diff-sbs-add"}`}
        classList={{ "diff-line-selected": selected() }}
        onClick={() => props.onToggleLine(props.hunkIndex, index)}
      >
        {hasBothSides ? <WordDiffSpans spans={wordSpans()!} /> : <SyntaxSpans text={text} />}
      </button>
    );
  }

  return (
    <For each={rows()}>
      {(row) => (
        <div class="diff-sbs-row">
          {cell(row, "left")}
          {cell(row, "right")}
        </div>
      )}
    </For>
  );
}

function HunkBlock(props: {
  hunk: Hunk;
  hunkIndex: number;
  staged: boolean;
  selected: Set<number>;
  mode: "inline" | "side-by-side";
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
      <Show
        when={props.mode === "inline"}
        fallback={
          <SideBySideHunkBody
            hunk={props.hunk}
            hunkIndex={props.hunkIndex}
            selected={props.selected}
            onToggleLine={props.onToggleLine}
          />
        }
      >
        <InlineHunkBody
          hunk={props.hunk}
          hunkIndex={props.hunkIndex}
          selected={props.selected}
          onToggleLine={props.onToggleLine}
        />
      </Show>
    </div>
  );
}

export const HunkDiffView: Component<{
  root: string;
  path: string;
  staged: boolean;
  onChanged: () => void;
}> = (props) => {
  const [mode, setMode] = createSignal<"inline" | "side-by-side">("inline");
  const [ignoreWhitespace, setIgnoreWhitespace] = createSignal(false);
  const [fullFile, setFullFile] = createSignal(false);
  const [tabWidth, setTabWidth] = createSignal(4);

  const [diff, { refetch }] = createResource(
    () => [props.root, props.path, props.staged, ignoreWhitespace(), fullFile()] as const,
    ([root, path, staged, ignoreWs, full]) =>
      getFileDiffWithOptions(root, path, staged, full ? FULL_FILE_CONTEXT : undefined, ignoreWs),
  );
  const [selected, setSelected] = createSignal<Set<number>>(new Set());
  const [applyError, setApplyError] = createSignal<string | null>(null);
  const [allowLargeRender, setAllowLargeRender] = createSignal(false);

  const totalLines = createMemo(() =>
    (diff()?.hunks ?? []).reduce((sum, hunk) => sum + hunk.lines.length, 0),
  );
  const isLarge = createMemo(() => totalLines() > LARGE_DIFF_LINE_THRESHOLD);

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

  async function runApply(action: () => Promise<void>) {
    try {
      await action();
      setApplyError(null);
    } catch (err) {
      setApplyError(String(err));
    } finally {
      await afterChange();
    }
  }

  function stageHunk(index: number) {
    return runApply(() => stageFileHunks(props.root, props.path, [index]));
  }

  function unstageHunk(index: number) {
    return runApply(() => unstageFileHunks(props.root, props.path, [index]));
  }

  function stageSelected(hunkIndex: number) {
    const lines = [...selected()]
      .filter((k) => Math.floor(k / 100000) === hunkIndex)
      .map((k) => k % 100000);
    return runApply(() => stageFileLines(props.root, props.path, hunkIndex, lines));
  }

  function unstageSelected(hunkIndex: number) {
    const lines = [...selected()]
      .filter((k) => Math.floor(k / 100000) === hunkIndex)
      .map((k) => k % 100000);
    return runApply(() => unstageFileLines(props.root, props.path, hunkIndex, lines));
  }

  return (
    <div class="diff-view" style={{ "--diff-tab-size": String(tabWidth()) }}>
      <Show when={applyError()}>
        <p class="diff-apply-error">{applyError()}</p>
      </Show>
      <Show when={diff.loading}>
        <p class="text-muted">Loading diff…</p>
      </Show>
      <Show when={diff() && diff()!.non_textual}>
        {(kind) => <NonTextualDiffView root={props.root} path={props.path} staged={props.staged} kind={kind()} />}
      </Show>
      <Show when={diff() && !diff()!.non_textual}>
        <div class="diff-toolbar">
          <div class="diff-mode-toggle">
            <button
              type="button"
              classList={{ "diff-mode-active": mode() === "inline" }}
              onClick={() => setMode("inline")}
            >
              Inline
            </button>
            <button
              type="button"
              classList={{ "diff-mode-active": mode() === "side-by-side" }}
              onClick={() => setMode("side-by-side")}
            >
              Side by side
            </button>
          </div>
          <label class="diff-toolbar-option">
            <input type="checkbox" checked={ignoreWhitespace()} onChange={(e) => setIgnoreWhitespace(e.currentTarget.checked)} />
            Ignore whitespace
          </label>
          <label class="diff-toolbar-option">
            <input type="checkbox" checked={fullFile()} onChange={(e) => setFullFile(e.currentTarget.checked)} />
            Show full file
          </label>
          <label class="diff-toolbar-option">
            Tab width
            <input
              class="diff-tab-width-input"
              type="number"
              min="1"
              max="8"
              value={tabWidth()}
              onInput={(e) => setTabWidth(Math.max(1, Math.min(8, Number(e.currentTarget.value) || 4)))}
            />
          </label>
        </div>
        <Show
          when={!isLarge() || allowLargeRender()}
          fallback={
            <div class="diff-large-gate">
              <p class="text-muted">Large diff ({totalLines().toLocaleString()} lines) — not rendered by default.</p>
              <button type="button" class="working-copy-action" onClick={() => setAllowLargeRender(true)}>
                Show anyway
              </button>
            </div>
          }
        >
          <For each={diff()?.hunks ?? []}>
            {(hunk, i) => (
              <HunkBlock
                hunk={hunk}
                hunkIndex={i()}
                staged={props.staged}
                selected={selected()}
                mode={mode()}
                onToggleLine={toggleLine}
                onAction={() => (props.staged ? unstageHunk(i()) : stageHunk(i()))}
                onActionSelected={() => (props.staged ? unstageSelected(i()) : stageSelected(i()))}
              />
            )}
          </For>
        </Show>
      </Show>
    </div>
  );
};
