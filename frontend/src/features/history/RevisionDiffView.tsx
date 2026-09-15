import { type Component, For, Show, createMemo, createResource, createSignal } from "solid-js";
import { getRevisionDiff } from "../../api/commands";
import type { FileDiff, Hunk } from "../../api/types";
import { highlightLine, type Token } from "../../diff/highlight";
import { pairHunkLines, type PairedRow } from "../../diff/side-by-side";
import { wordDiff } from "../../diff/word-diff";
import { NonTextualDiffView } from "../working-copy/NonTextualDiffView";

const FULL_FILE_CONTEXT = 1_000_000;
const LARGE_DIFF_LINE_THRESHOLD = 2000;

function isChangedLine(line: string): boolean {
  return line.startsWith("+") || line.startsWith("-");
}

function lineClass(line: string): string {
  if (line.startsWith("+")) return "diff-line diff-line-add";
  if (line.startsWith("-")) return "diff-line diff-line-remove";
  return "diff-line";
}

function SyntaxSpans(props: { text: string }) {
  return (
    <For each={highlightLine(props.text)}>
      {(tok: Token) => (tok.class ? <span class={tok.class}>{tok.text}</span> : <>{tok.text}</>)}
    </For>
  );
}

function WordDiffSpans(props: { spans: { text: string; changed: boolean }[] }) {
  return (
    <For each={props.spans}>
      {(span) => (span.changed ? <mark class="diff-word-changed">{span.text}</mark> : <>{span.text}</>)}
    </For>
  );
}

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

function RevisionInlineHunk(props: { hunk: Hunk }) {
  const paired = createMemo(() => pairedChangeIndices(props.hunk.lines));
  return (
    <For each={props.hunk.lines}>
      {(line, i) => {
        const changed = isChangedLine(line);
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
            <div class={lineClass(line)}>
              <SyntaxSpans text={line.startsWith(" ") ? line.slice(1) : line} />
            </div>
          );
        }
        return (
          <div class={lineClass(line)}>
            <span class="diff-line-prefix">{line[0]}</span>
            {content()}
          </div>
        );
      }}
    </For>
  );
}

function RevisionSideBySideHunk(props: { hunk: Hunk }) {
  const rows = createMemo(() => pairHunkLines(props.hunk.lines));

  function cell(row: PairedRow, side: "left" | "right") {
    const text = side === "left" ? row.leftText : row.rightText;
    if (text === null) {
      return <div class="diff-sbs-cell diff-sbs-empty" />;
    }
    const isRemove = side === "left" && row.rightText !== null;
    const isAdd = side === "right" && row.leftText !== null;
    const isUnpairedRemove = side === "left" && row.rightText === null;
    const isUnpairedAdd = side === "right" && row.leftText === null;

    let cls = "diff-sbs-cell";
    if (isRemove || isUnpairedRemove) cls += " diff-line-remove";
    if (isAdd || isUnpairedAdd) cls += " diff-line-add";

    const content = () => {
      if ((isRemove || isAdd) && row.leftText !== null && row.rightText !== null) {
        const { oldSpans, newSpans } = wordDiff(row.leftText, row.rightText);
        return <WordDiffSpans spans={side === "left" ? oldSpans : newSpans} />;
      }
      return <SyntaxSpans text={text} />;
    };

    return (
      <div class={cls}>
        <span class="diff-line-prefix">{side === "left" ? (row.rightText !== null ? "-" : "-") : (row.leftText !== null ? "+" : "+")}</span>
        {content()}
      </div>
    );
  }

  return (
    <div class="diff-sbs-grid">
      <For each={rows()}>
        {(row) => (
          <div class="diff-sbs-row">
            {cell(row, "left")}
            {cell(row, "right")}
          </div>
        )}
      </For>
    </div>
  );
}

export const RevisionDiffView: Component<{
  root: string;
  oldRev: string;
  oldPath?: string | null;
  newRev: string;
  newPath?: string | null;
  onSwap?: () => void;
  onClose: () => void;
}> = (props) => {
  const [mode, setMode] = createSignal<"inline" | "side-by-side">("inline");
  const [ignoreWhitespace, setIgnoreWhitespace] = createSignal(false);
  const [fullFile, setFullFile] = createSignal(false);
  const [tabWidth, setTabWidth] = createSignal(4);
  const [allowLargeRender, setAllowLargeRender] = createSignal(false);
  const [selectedFileIndex, setSelectedFileIndex] = createSignal(0);

  const [diff] = createResource(
    () => [props.root, props.oldRev, props.oldPath, props.newRev, props.newPath, fullFile(), ignoreWhitespace()] as const,
    ([root, oldR, oldP, newR, newP, full, ignoreWs]) =>
      getRevisionDiff(root, oldR, oldP, newR, newP, full ? FULL_FILE_CONTEXT : undefined, ignoreWs),
  );

  const files = () => diff() ?? [];
  const currentFile = () => files()[selectedFileIndex()] ?? files()[0];

  const totalLines = createMemo(() =>
    (currentFile()?.hunks ?? []).reduce((sum, hunk) => sum + hunk.lines.length, 0),
  );
  const isLarge = createMemo(() => totalLines() > LARGE_DIFF_LINE_THRESHOLD);

  return (
    <div class="revision-diff-view" style={{ "--diff-tab-size": String(tabWidth()) }}>
      <div class="revision-diff-toolbar">
        <div class="revision-diff-revs">
          <span>Comparing <strong>{props.oldRev.slice(0, 7)}</strong> ↔ <strong>{props.newRev.slice(0, 7)}</strong></span>
          <Show when={props.oldPath || props.newPath}>
            <span class="text-muted">
              ({props.oldPath ?? props.newPath}
              {props.oldPath && props.newPath && props.oldPath !== props.newPath ? ` → ${props.newPath}` : ""})
            </span>
          </Show>
          <Show when={props.onSwap}>
            <button class="collapse-toggle" onClick={props.onSwap} title="Swap revision order">
              ⇄ Swap
            </button>
          </Show>
        </div>
        <div class="diff-toolbar">
          <div class="diff-mode-toggle" role="radiogroup" aria-label="Diff presentation">
            <button
              type="button"
              class="diff-mode-button"
              classList={{ "diff-mode-active": mode() === "inline" }}
              onClick={() => setMode("inline")}
            >
              Inline
            </button>
            <button
              type="button"
              class="diff-mode-button"
              classList={{ "diff-mode-active": mode() === "side-by-side" }}
              onClick={() => setMode("side-by-side")}
            >
              Side-by-side
            </button>
          </div>
          <label class="diff-toolbar-option">
            <input
              type="checkbox"
              checked={ignoreWhitespace()}
              onChange={(e) => setIgnoreWhitespace(e.currentTarget.checked)}
            />
            Ignore whitespace
          </label>
          <label class="diff-toolbar-option">
            <input
              type="checkbox"
              checked={fullFile()}
              onChange={(e) => setFullFile(e.currentTarget.checked)}
            />
            Show full file
          </label>
          <label class="diff-toolbar-option">
            Tab:
            <input
              type="number"
              min="1"
              max="8"
              value={tabWidth()}
              onInput={(e) => setTabWidth(Math.max(1, Math.min(8, Number(e.currentTarget.value) || 4)))}
            />
          </label>
          <button class="collapse-toggle" onClick={props.onClose}>
            Close diff
          </button>
        </div>
      </div>

      <Show when={diff.loading}>
        <p class="text-muted">Loading diff between revisions…</p>
      </Show>
      <Show when={diff.error}>
        <p class="diff-apply-error">{String(diff.error)}</p>
      </Show>

      <Show when={!diff.loading && files().length === 0}>
        <p class="text-muted">No differences between selected revisions.</p>
      </Show>

      <Show when={files().length > 1}>
        <div class="revision-diff-files-tab">
          <For each={files()}>
            {(file: FileDiff, i) => (
              <button
                class="collapse-toggle"
                classList={{ "diff-mode-active": selectedFileIndex() === i() }}
                onClick={() => setSelectedFileIndex(i())}
              >
                {file.path ?? `File ${i() + 1}`}
              </button>
            )}
          </For>
        </div>
      </Show>

      <Show when={currentFile()}>
        {(file) => (
          <Show
            when={!file().non_textual}
            fallback={
              <NonTextualDiffView
                root={props.root}
                path={file().path ?? props.newPath ?? props.oldPath ?? ""}
                staged={false}
                kind={file().non_textual!}
              />
            }
          >
            <Show
              when={!isLarge() || allowLargeRender()}
              fallback={
                <div class="large-diff-gate">
                  <p class="text-muted">
                    Large diff: {totalLines().toLocaleString()} lines changed. Rendering by default is suppressed for responsiveness.
                  </p>
                  <button class="working-copy-action" onClick={() => setAllowLargeRender(true)}>
                    Show anyway
                  </button>
                </div>
              }
            >
              <div class="hunk-diff">
                <For each={file().hunks}>
                  {(hunk) => (
                    <div class="hunk">
                      <div class="hunk-header">
                        <span class="hunk-header-text">{hunk.header}</span>
                      </div>
                      <Show
                        when={mode() === "inline"}
                        fallback={<RevisionSideBySideHunk hunk={hunk} />}
                      >
                        <RevisionInlineHunk hunk={hunk} />
                      </Show>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </Show>
        )}
      </Show>
    </div>
  );
};
