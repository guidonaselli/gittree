import { type Component, For, Show, createMemo } from "solid-js";
import {
  changeCodeLabel,
  isKnown,
  unknownReason,
  type ChangedEntry,
  type Resolved,
  type WorkingCopyStatus,
} from "../../api/types";

function entryPathset(entry: ChangedEntry): string[] {
  return entry.rename_or_copy_from ? [entry.path, entry.rename_or_copy_from[0]] : [entry.path];
}

function EntryRow(props: {
  entry: ChangedEntry;
  code: string;
  actionLabel: string;
  onAction: (paths: string[]) => void;
}) {
  return (
    <div class="working-copy-entry">
      <span class="working-copy-code">{props.code}</span>
      <span class="working-copy-path">{props.entry.path}</span>
      <Show when={props.entry.rename_or_copy_from}>
        <span class="text-muted">
          from {props.entry.rename_or_copy_from![0]} ({props.entry.rename_or_copy_from![1]}%)
        </span>
      </Show>
      <Show when={props.entry.submodule}>
        <span class="badge badge-warning">submodule</span>
      </Show>
      <button class="working-copy-action" onClick={() => props.onAction(entryPathset(props.entry))}>
        {props.actionLabel}
      </button>
    </div>
  );
}

export const WorkingCopyView: Component<{
  status: Resolved<WorkingCopyStatus>;
  onStage: (paths: string[]) => void;
  onUnstage: (paths: string[]) => void;
}> = (props) => {
  const known = createMemo(() => (isKnown(props.status) ? props.status.value : null));
  const reason = createMemo(() => unknownReason(props.status));

  const staged = createMemo(() => known()?.changed.filter((e) => e.staged !== "Unmodified") ?? []);
  const unstaged = createMemo(() => known()?.changed.filter((e) => e.unstaged !== "Unmodified") ?? []);
  const untracked = createMemo(() => known()?.untracked ?? []);
  const conflicted = createMemo(() => known()?.conflicted ?? []);

  const allUnstagedPaths = createMemo(() => [
    ...unstaged().flatMap(entryPathset),
    ...untracked(),
  ]);
  const allStagedPaths = createMemo(() => staged().flatMap(entryPathset));

  return (
    <Show when={known()} fallback={<div class="text-muted">working copy status unknown ({reason()})</div>}>
      <div class="working-copy">
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
        <section class="working-copy-group">
          <div class="working-copy-group-header">
            <h3>Staged ({staged().length})</h3>
            <Show when={staged().length > 0}>
              <button class="working-copy-action" onClick={() => props.onUnstage(allStagedPaths())}>
                Unstage all
              </button>
            </Show>
          </div>
          <For each={staged()}>
            {(e) => (
              <EntryRow
                entry={e}
                code={changeCodeLabel(e.staged)}
                actionLabel="Unstage"
                onAction={props.onUnstage}
              />
            )}
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
          <For each={unstaged()}>
            {(e) => (
              <EntryRow entry={e} code={changeCodeLabel(e.unstaged)} actionLabel="Stage" onAction={props.onStage} />
            )}
          </For>
        </section>
        <section class="working-copy-group">
          <h3>Untracked ({untracked().length})</h3>
          <For each={untracked()}>
            {(path) => (
              <div class="working-copy-entry">
                <span class="working-copy-code">?</span>
                <span class="working-copy-path">{path}</span>
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
