import {
  type Component,
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type JSX,
} from "solid-js";
import {
  changeCodeLabel,
  isKnown,
  unknownReason,
  type ChangedEntry,
  type Resolved,
  type WorkingCopyStatus,
} from "../../api/types";
import type { CommitMessageTemplate, CommitOptions } from "../../api/commands";
import { BlameView } from "./BlameView";
import { CommitPanel } from "./CommitPanel";
import { HunkDiffView } from "./HunkDiffView";

function entryPathset(entry: ChangedEntry): string[] {
  return entry.rename_or_copy_from ? [entry.path, entry.rename_or_copy_from[0]] : [entry.path];
}

type Selection = { path: string; staged: boolean } | null;
type DiscardTarget = { paths: string[]; label: string; untracked: boolean } | null;

const ROW_HEIGHT = 28;
const OVERSCAN = 15;

function DiscardConfirm(props: {
  target: NonNullable<DiscardTarget>;
  onConfirm: () => void;
  onStash: () => void;
  onCancel: () => void;
}) {
  return (
    <div class="discard-confirm">
      <p>
        {props.target.untracked
          ? `Permanently delete ${props.target.label}? Untracked files cannot be recovered from git.`
          : `Discard changes to ${props.target.label}? This cannot be undone.`}
      </p>
      <div class="discard-confirm-actions">
        <button class="working-copy-action" onClick={props.onStash}>
          Stash instead
        </button>
        <button class="working-copy-action working-copy-action-danger" onClick={props.onConfirm}>
          {props.target.untracked ? "Delete" : "Discard"}
        </button>
        <button class="working-copy-action" onClick={props.onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}

function EntryRow(props: {
  entry: ChangedEntry;
  code: string;
  actionLabel: string;
  onAction: (paths: string[]) => void;
  onTogglePath: () => void;
  expanded: boolean;
  onDiscard?: () => void;
  onToggleBlame: () => void;
  blameShown: boolean;
  onViewHistory?: () => void;
}) {
  return (
    <div class="working-copy-entry" classList={{ "working-copy-entry-expanded": props.expanded }}>
      <span class="working-copy-code">{props.code}</span>
      <button class="working-copy-path working-copy-path-button" onClick={props.onTogglePath}>
        {props.entry.path}
      </button>
      <Show when={props.entry.rename_or_copy_from}>
        <span class="text-muted">
          from {props.entry.rename_or_copy_from![0]} ({props.entry.rename_or_copy_from![1]}%)
        </span>
      </Show>
      <Show when={props.entry.submodule}>
        <span class="badge badge-warning">submodule</span>
      </Show>
      <Show when={props.onDiscard}>
        <button class="working-copy-action" onClick={props.onDiscard}>
          Discard
        </button>
      </Show>
      <button class="working-copy-action" classList={{ "diff-mode-active": props.blameShown }} onClick={props.onToggleBlame}>
        Blame
      </button>
      <Show when={props.onViewHistory}>
        <button class="working-copy-action" onClick={props.onViewHistory}>
          History
        </button>
      </Show>
      <button class="working-copy-action" onClick={() => props.onAction(entryPathset(props.entry))}>
        {props.actionLabel}
      </button>
    </div>
  );
}

function VirtualFileList<T>(props: {
  items: T[];
  containerRef: () => HTMLDivElement | undefined;
  scrollTop: () => number;
  viewportHeight: () => number;
  renderItem: (item: T) => JSX.Element;
  activeItemKey?: string | null;
  getItemKey: (item: T) => string;
}) {
  let listRef: HTMLDivElement | undefined;

  const visibleRange = createMemo(() => {
    const total = props.items.length;
    if (total <= 40) {
      return { start: 0, end: total };
    }
    const container = props.containerRef();
    if (!container || !listRef) {
      return { start: 0, end: Math.min(total, 50) };
    }

    const sTop = props.scrollTop();
    const vHeight = props.viewportHeight();

    let offset = 0;
    let el: HTMLElement | null = listRef;
    while (el && el !== container) {
      offset += el.offsetTop;
      el = el.offsetParent as HTMLElement | null;
    }

    const relStartPx = Math.max(0, sTop - offset);
    const relEndPx = Math.max(0, sTop + vHeight - offset);

    let start = Math.max(0, Math.floor(relStartPx / ROW_HEIGHT) - OVERSCAN);
    let end = Math.min(total, Math.ceil(relEndPx / ROW_HEIGHT) + OVERSCAN);

    if (props.activeItemKey) {
      const activeIdx = props.items.findIndex(
        (it) => props.getItemKey(it) === props.activeItemKey
      );
      if (activeIdx >= 0) {
        if (activeIdx < start) start = activeIdx;
        if (activeIdx >= end) end = activeIdx + 1;
      }
    }

    return { start, end };
  });

  const topSpacerHeight = () => visibleRange().start * ROW_HEIGHT;
  const bottomSpacerHeight = () => (props.items.length - visibleRange().end) * ROW_HEIGHT;
  const renderedItems = createMemo(() =>
    props.items.slice(visibleRange().start, visibleRange().end)
  );

  return (
    <div ref={listRef} class="virtual-file-list">
      <Show when={topSpacerHeight() > 0}>
        <div style={{ height: `${topSpacerHeight()}px` }} />
      </Show>
      <For each={renderedItems()}>
        {(item) => props.renderItem(item)}
      </For>
      <Show when={bottomSpacerHeight() > 0}>
        <div style={{ height: `${bottomSpacerHeight()}px` }} />
      </Show>
    </div>
  );
}

export const WorkingCopyView: Component<{
  root: string;
  status: Resolved<WorkingCopyStatus>;
  isScanning?: boolean;
  onCancelScan?: () => void;
  onStage: (paths: string[]) => void;
  onUnstage: (paths: string[]) => void;
  onDiscard: (paths: string[]) => void;
  onDeleteUntracked: (paths: string[]) => void;
  onStash: (paths: string[]) => void;
  onHunksChanged: () => void;
  onCommit: (message: string, options: CommitOptions) => Promise<void>;
  commitError: string | null;
  hookOutput: string | null;
  messageTemplate: CommitMessageTemplate | null;
  onCheckHeadPublished: () => Promise<boolean>;
  onAmend: (message: string, options: CommitOptions) => Promise<void>;
  onViewHistory?: (path: string) => void;
}> = (props) => {
  let containerRef: HTMLDivElement | undefined;
  const [scrollTop, setScrollTop] = createSignal(0);
  const [viewportHeight, setViewportHeight] = createSignal(800);

  const [selection, setSelection] = createSignal<Selection>(null);
  const [discardTarget, setDiscardTarget] = createSignal<DiscardTarget>(null);
  const [blamePath, setBlamePath] = createSignal<string | null>(null);

  function handleScroll(e: Event) {
    const target = e.currentTarget as HTMLElement;
    setScrollTop(target.scrollTop);
  }

  onMount(() => {
    if (containerRef) {
      setViewportHeight(containerRef.clientHeight || 800);
      const observer = new ResizeObserver((entries) => {
        for (const entry of entries) {
          setViewportHeight(entry.contentRect.height || 800);
        }
      });
      observer.observe(containerRef);
      onCleanup(() => observer.disconnect());
    }
  });

  function toggle(path: string, staged: boolean) {
    const current = selection();
    if (current && current.path === path && current.staged === staged) setSelection(null);
    else setSelection({ path, staged });
  }

  function toggleBlame(path: string) {
    setBlamePath(blamePath() === path ? null : path);
  }

  function confirmDiscard() {
    const target = discardTarget();
    if (!target) return;
    if (target.untracked) props.onDeleteUntracked(target.paths);
    else props.onDiscard(target.paths);
    setDiscardTarget(null);
  }

  function confirmStash() {
    const target = discardTarget();
    if (!target) return;
    props.onStash(target.paths);
    setDiscardTarget(null);
  }

  const known = createMemo(() => (isKnown(props.status) ? props.status.value : null));
  const reason = createMemo(() => unknownReason(props.status));

  const staged = createMemo(() => known()?.changed.filter((e) => e.staged !== "Unmodified") ?? []);
  const unstaged = createMemo(() => known()?.changed.filter((e) => e.unstaged !== "Unmodified") ?? []);
  const untracked = createMemo(() => known()?.untracked ?? []);
  const conflicted = createMemo(() => known()?.conflicted ?? []);

  const allUnstagedPaths = createMemo(() => [...unstaged().flatMap(entryPathset), ...untracked()]);
  const allStagedPaths = createMemo(() => staged().flatMap(entryPathset));

  const [admittedCount, setAdmittedCount] = createSignal(100);

  createEffect(() => {
    const statusVal = known();
    if (!statusVal) return;
    const total = statusVal.changed.length + statusVal.untracked.length;
    if (total <= 100) {
      setAdmittedCount(total);
      return;
    }
    setAdmittedCount(100);
    let current = 100;
    let rafId: number;

    function step() {
      current = Math.min(total, current + 500);
      setAdmittedCount(current);
      if (current < total) {
        rafId = requestAnimationFrame(step);
      }
    }

    rafId = requestAnimationFrame(step);
    onCleanup(() => cancelAnimationFrame(rafId));
  });

  const stagedItems = createMemo(() =>
    staged().slice(0, Math.min(staged().length, admittedCount()))
  );
  const unstagedOffset = createMemo(() => Math.max(0, admittedCount() - staged().length));
  const unstagedItems = createMemo(() =>
    unstaged().slice(0, Math.min(unstaged().length, unstagedOffset()))
  );
  const untrackedOffset = createMemo(() =>
    Math.max(0, unstagedOffset() - unstaged().length)
  );
  const untrackedItems = createMemo(() =>
    untracked().slice(0, Math.min(untracked().length, untrackedOffset()))
  );

  return (
    <Show when={known()} fallback={<div class="text-muted">working copy status unknown ({reason()})</div>}>
      <div ref={containerRef} class="working-copy" onScroll={handleScroll}>
        <Show when={props.isScanning}>
          <div class="working-copy-scan-banner">
            <span class="working-copy-scan-text">Scanning working copy...</span>
            <Show when={props.onCancelScan}>
              <button class="working-copy-action" onClick={props.onCancelScan}>
                Cancel scan
              </button>
            </Show>
          </div>
        </Show>
        <Show when={discardTarget()}>
          {(target) => (
            <DiscardConfirm target={target()} onConfirm={confirmDiscard} onStash={confirmStash} onCancel={() => setDiscardTarget(null)} />
          )}
        </Show>
        <Show when={conflicted().length > 0}>
          <section class="working-copy-group">
            <h3 class="text-danger">Conflicted ({conflicted().length})</h3>
            <For each={conflicted()}>
              {(c) => (
                <div class="working-copy-entry">
                  <span class="working-copy-code">{c.code}</span>
                  <span class="working-copy-path">{c.path}</span>
                </div>
              )}
            </For>
          </section>
        </Show>
        <CommitPanel
          root={props.root}
          stagedCount={staged().length}
          onCommit={props.onCommit}
          error={props.commitError}
          hookOutput={props.hookOutput}
          messageTemplate={props.messageTemplate}
          onCheckHeadPublished={props.onCheckHeadPublished}
          onAmend={props.onAmend}
        />
        <section class="working-copy-group">
          <div class="working-copy-group-header">
            <h3>Staged ({staged().length})</h3>
            <Show when={staged().length > 0}>
              <button class="working-copy-action" onClick={() => props.onUnstage(allStagedPaths())}>
                Unstage all
              </button>
            </Show>
          </div>
          <VirtualFileList
            items={stagedItems()}
            containerRef={() => containerRef}
            scrollTop={scrollTop}
            viewportHeight={viewportHeight}
            activeItemKey={selection()?.staged ? selection()?.path : blamePath()}
            getItemKey={(e) => e.path}
            renderItem={(entry) => (
              <>
                <EntryRow
                  entry={entry}
                  code={changeCodeLabel(entry.staged)}
                  actionLabel="Unstage"
                  onAction={props.onUnstage}
                  onTogglePath={() => toggle(entry.path, true)}
                  expanded={selection()?.path === entry.path && selection()?.staged === true}
                  onToggleBlame={() => toggleBlame(entry.path)}
                  blameShown={blamePath() === entry.path}
                  onViewHistory={props.onViewHistory ? () => props.onViewHistory!(entry.path) : undefined}
                />
                <Show when={selection()?.path === entry.path && selection()?.staged === true}>
                  <HunkDiffView root={props.root} path={entry.path} staged={true} onChanged={props.onHunksChanged} />
                </Show>
                <Show when={blamePath() === entry.path}>
                  <BlameView root={props.root} path={entry.path} />
                </Show>
              </>
            )}
          />
        </section>
        <section class="working-copy-group">
          <div class="working-copy-group-header">
            <h3>Unstaged ({unstaged().length})</h3>
            <Show when={allUnstagedPaths().length > 0}>
              <button class="working-copy-action" onClick={() => props.onStage(allUnstagedPaths())}>
                Stage all
              </button>
            </Show>
          </div>
          <VirtualFileList
            items={unstagedItems()}
            containerRef={() => containerRef}
            scrollTop={scrollTop}
            viewportHeight={viewportHeight}
            activeItemKey={selection() && !selection()?.staged ? selection()?.path : blamePath()}
            getItemKey={(e) => e.path}
            renderItem={(entry) => (
              <>
                <EntryRow
                  entry={entry}
                  code={changeCodeLabel(entry.unstaged)}
                  actionLabel="Stage"
                  onAction={props.onStage}
                  onTogglePath={() => toggle(entry.path, false)}
                  expanded={selection()?.path === entry.path && selection()?.staged === false}
                  onDiscard={() => setDiscardTarget({ paths: entryPathset(entry), label: entry.path, untracked: false })}
                  onToggleBlame={() => toggleBlame(entry.path)}
                  blameShown={blamePath() === entry.path}
                  onViewHistory={props.onViewHistory ? () => props.onViewHistory!(entry.path) : undefined}
                />
                <Show when={selection()?.path === entry.path && selection()?.staged === false}>
                  <HunkDiffView root={props.root} path={entry.path} staged={false} onChanged={props.onHunksChanged} />
                </Show>
                <Show when={blamePath() === entry.path}>
                  <BlameView root={props.root} path={entry.path} />
                </Show>
              </>
            )}
          />
        </section>
        <section class="working-copy-group">
          <h3>Untracked ({untracked().length})</h3>
          <VirtualFileList
            items={untrackedItems()}
            containerRef={() => containerRef}
            scrollTop={scrollTop}
            viewportHeight={viewportHeight}
            getItemKey={(path) => path}
            renderItem={(path) => (
              <div class="working-copy-entry">
                <span class="working-copy-code">?</span>
                <span class="working-copy-path">{path}</span>
                <button
                  class="working-copy-action"
                  onClick={() => setDiscardTarget({ paths: [path], label: path, untracked: true })}
                >
                  Delete
                </button>
                <button class="working-copy-action" onClick={() => props.onStage([path])}>
                  Stage
                </button>
              </div>
            )}
          />
        </section>
      </div>
    </Show>
  );
};
