import { type Component, For, Show, createMemo, createSignal } from "solid-js";
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
      <button class="working-copy-action" onClick={() => props.onAction(entryPathset(props.entry))}>
        {props.actionLabel}
      </button>
    </div>
  );
}

export const WorkingCopyView: Component<{
  root: string;
  status: Resolved<WorkingCopyStatus>;
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
}> = (props) => {
  const [selection, setSelection] = createSignal<Selection>(null);
  const [discardTarget, setDiscardTarget] = createSignal<DiscardTarget>(null);
  const [blamePath, setBlamePath] = createSignal<string | null>(null);

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

  return (
    <Show when={known()} fallback={<div class="text-muted">working copy status unknown ({reason()})</div>}>
      <div class="working-copy">
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
          <For each={staged().map((e) => e.path)}>
            {(path) => {
              const entry = createMemo(() => staged().find((x) => x.path === path)!);
              return (
                <>
                  <EntryRow
                    entry={entry()}
                    code={changeCodeLabel(entry().staged)}
                    actionLabel="Unstage"
                    onAction={props.onUnstage}
                    onTogglePath={() => toggle(path, true)}
                    expanded={selection()?.path === path && selection()?.staged === true}
                    onToggleBlame={() => toggleBlame(path)}
                    blameShown={blamePath() === path}
                  />
                  <Show when={selection()?.path === path && selection()?.staged === true}>
                    <HunkDiffView root={props.root} path={path} staged={true} onChanged={props.onHunksChanged} />
                  </Show>
                  <Show when={blamePath() === path}>
                    <BlameView root={props.root} path={path} />
                  </Show>
                </>
              );
            }}
          </For>
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
          <For each={unstaged().map((e) => e.path)}>
            {(path) => {
              const entry = createMemo(() => unstaged().find((x) => x.path === path)!);
              return (
                <>
                  <EntryRow
                    entry={entry()}
                    code={changeCodeLabel(entry().unstaged)}
                    actionLabel="Stage"
                    onAction={props.onStage}
                    onTogglePath={() => toggle(path, false)}
                    expanded={selection()?.path === path && selection()?.staged === false}
                    onDiscard={() => setDiscardTarget({ paths: entryPathset(entry()), label: path, untracked: false })}
                    onToggleBlame={() => toggleBlame(path)}
                    blameShown={blamePath() === path}
                  />
                  <Show when={selection()?.path === path && selection()?.staged === false}>
                    <HunkDiffView root={props.root} path={path} staged={false} onChanged={props.onHunksChanged} />
                  </Show>
                  <Show when={blamePath() === path}>
                    <BlameView root={props.root} path={path} />
                  </Show>
                </>
              );
            }}
          </For>
        </section>
        <section class="working-copy-group">
          <h3>Untracked ({untracked().length})</h3>
          <For each={untracked()}>
            {(path) => (
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
          </For>
        </section>
      </div>
    </Show>
  );
};
