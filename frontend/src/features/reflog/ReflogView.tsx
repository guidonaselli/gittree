import {
  type Component,
  createSignal,
  createResource,
  createMemo,
  For,
  Show,
  onMount,
  onCleanup,
} from "solid-js";
import { getReflog, resetToReflog, getBranches, createBranch } from "../../api/commands";
import type { ReflogEntry, ResetMode, ResetOutcome } from "../../api/types";
import { ResetConfirmationModal } from "./ResetConfirmationModal";
import "./reflog.css";
import { Icon } from "../../ui/Icon";

export interface ReflogViewProps {
  root: string;
  initialRef?: string;
  onSelectCommit?: (sha: string) => void;
  onResetSuccess?: (outcome: ResetOutcome) => void;
}

export const ReflogView: Component<ReflogViewProps> = (props) => {
  const [selectedRef, setSelectedRef] = createSignal<string>(props.initialRef ?? "HEAD");
  const [limit, setLimit] = createSignal<number>(100);
  const [filterQuery, setFilterQuery] = createSignal<string>("");
  const [entryToReset, setEntryToReset] = createSignal<ReflogEntry | null>(null);
  const [creatingBranchFor, setCreatingBranchFor] = createSignal<ReflogEntry | null>(null);
  const [newBranchName, setNewBranchName] = createSignal<string>("");
  const [branchError, setBranchError] = createSignal<string | null>(null);
  const [statusNotice, setStatusNotice] = createSignal<string | null>(null);

  onMount(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && creatingBranchFor()) {
        setCreatingBranchFor(null);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    onCleanup(() => window.removeEventListener("keydown", onKeyDown));
  });

  const [branches] = createResource(() => props.root, async (root) => {
    try {
      const list = await getBranches(root);
      return list.filter((b) => !b.is_remote);
    } catch {
      return [];
    }
  });

  const [reflogEntries, { refetch: refetchReflog }] = createResource(
    () => ({ root: props.root, ref: selectedRef(), limit: limit() }),
    async ({ root, ref, limit }) => {
      return await getReflog(root, ref, limit);
    }
  );

  const filteredEntries = createMemo(() => {
    const list = reflogEntries() ?? [];
    const query = filterQuery().trim().toLowerCase();
    if (!query) return list;
    return list.filter(
      (e) =>
        e.message.toLowerCase().includes(query) ||
        e.operation.toLowerCase().includes(query) ||
        e.short_sha.toLowerCase().includes(query) ||
        e.commit_sha.toLowerCase().includes(query) ||
        e.committer_name.toLowerCase().includes(query) ||
        e.selector.toLowerCase().includes(query)
    );
  });

  function getBadgeClass(operation: string): string {
    const op = operation.toLowerCase();
    if (op.startsWith("commit")) return "reflog-badge-commit";
    if (op.startsWith("checkout")) return "reflog-badge-checkout";
    if (op.startsWith("reset")) return "reflog-badge-reset";
    if (op.startsWith("rebase")) return "reflog-badge-rebase";
    if (op.startsWith("merge")) return "reflog-badge-merge";
    if (op.startsWith("pull")) return "reflog-badge-pull";
    if (op.startsWith("cherry-pick")) return "reflog-badge-cherry-pick";
    return "reflog-badge-other";
  }

  async function handleReset(mode: ResetMode) {
    const entry = entryToReset();
    if (!entry) return;

    const outcome = await resetToReflog(props.root, {
      target_ref: selectedRef(),
      commit_sha: entry.commit_sha,
      mode,
    });

    setEntryToReset(null);
    setStatusNotice(outcome.summary);
    refetchReflog();
    props.onResetSuccess?.(outcome);
  }

  async function handleCreateBranch() {
    const entry = creatingBranchFor();
    const name = newBranchName().trim();
    if (!entry || !name) return;

    setBranchError(null);
    try {
      await createBranch(props.root, name, entry.commit_sha);
      setCreatingBranchFor(null);
      setNewBranchName("");
      setStatusNotice(`Created branch '${name}' at ${entry.short_sha}`);
    } catch (err) {
      setBranchError(String(err));
    }
  }

  return (
    <div class="reflog-container">
      <div class="reflog-header">
        <div class="reflog-title-area">
          <h2 class="reflog-title">Reflog Recovery</h2>
          <span class="reflog-subtitle">
            Audit and restore reference movements for HEAD and branches
          </span>
        </div>

        <div class="reflog-controls">
          <label for="reflog-target-select" class="reflog-meta-cell">Ref:</label>
          <select
            id="reflog-target-select"
            class="reflog-select"
            value={selectedRef()}
            onChange={(e) => setSelectedRef(e.currentTarget.value)}
          >
            <option value="HEAD">HEAD</option>
            <For each={branches()}>
              {(branch) => <option value={branch.name}>{branch.name}</option>}
            </For>
          </select>

          <label for="reflog-limit-select" class="reflog-meta-cell">Limit:</label>
          <select
            id="reflog-limit-select"
            class="reflog-select"
            value={limit()}
            onChange={(e) => setLimit(Number(e.currentTarget.value))}
          >
            <option value={50}>50</option>
            <option value={100}>100</option>
            <option value={200}>200</option>
            <option value={500}>500</option>
          </select>

          <input
            type="text"
            class="reflog-input"
            placeholder="Filter reflog entries..."
            value={filterQuery()}
            onInput={(e) => setFilterQuery(e.currentTarget.value)}
          />

          <button class="collapse-toggle" onClick={() => refetchReflog()}>
            Refresh
          </button>
        </div>
      </div>

      <Show when={statusNotice()}>
        {(notice) => (
          <div
            class="integration-notice"
            style={{
              padding: "var(--space-2) var(--space-4)",
              "background-color": "var(--color-bg-subtle)",
              "border": "var(--border-width-thin) solid var(--color-border)",
              "border-radius": "var(--radius-sm)",
              display: "flex",
              "align-items": "center",
              "justify-content": "space-between",
            }}
          >
            <span>{notice()}</span>
            <button class="collapse-toggle" onClick={() => setStatusNotice(null)}>
              Dismiss
            </button>
          </div>
        )}
      </Show>

      <div class="reflog-table-container">
        <Show when={reflogEntries.loading}>
          <div class="reflog-empty">Loading reflog entries...</div>
        </Show>

        <Show when={reflogEntries.error}>
          <div class="reflog-empty text-danger">
            Error loading reflog: {String(reflogEntries.error)}
          </div>
        </Show>

        <Show when={!reflogEntries.loading && !reflogEntries.error}>
          <Show
            when={filteredEntries().length > 0}
            fallback={<div class="reflog-empty">No reflog entries found for {selectedRef()}.</div>}
          >
            <table class="reflog-table">
              <thead>
                <tr>
                  <th class="reflog-th">Selector</th>
                  <th class="reflog-th">Action</th>
                  <th class="reflog-th">Commit</th>
                  <th class="reflog-th">Message</th>
                  <th class="reflog-th">Author & Date</th>
                  <th class="reflog-th reflog-actions-cell">Recovery</th>
                </tr>
              </thead>
              <tbody>
                <For each={filteredEntries()}>
                  {(entry) => (
                    <tr class="reflog-tr">
                      <td class="reflog-td reflog-selector">{entry.selector}</td>
                      <td class="reflog-td">
                        <span class={`reflog-badge ${getBadgeClass(entry.operation)}`}>
                          {entry.operation}
                        </span>
                      </td>
                      <td class="reflog-td">
                        <button
                          class="reflog-sha-btn"
                          title={`View commit ${entry.commit_sha}`}
                          onClick={() => props.onSelectCommit?.(entry.commit_sha)}
                        >
                          {entry.short_sha}
                        </button>
                      </td>
                      <td class="reflog-td reflog-message-cell" title={entry.message}>
                        {entry.message}
                      </td>
                      <td class="reflog-td reflog-meta-cell">
                        {entry.committer_name} • {entry.committer_date.split(" ")[0]}
                      </td>
                      <td class="reflog-td reflog-actions-cell">
                        <div class="reflog-row-actions">
                          <button
                            class="reflog-action-btn"
                            title={`Reset ${selectedRef()} to this point`}
                            onClick={() => setEntryToReset(entry)}
                          >
                            Reset to here
                          </button>
                          <button
                            class="reflog-action-btn"
                            title={`Create new branch at ${entry.short_sha}`}
                            onClick={() => {
                              setBranchError(null);
                              setNewBranchName("");
                              setCreatingBranchFor(entry);
                            }}
                          >
                            Branch here
                          </button>
                        </div>
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </Show>
        </Show>
      </div>

      <Show when={entryToReset()}>
        {(entry) => (
          <ResetConfirmationModal
            targetRef={selectedRef()}
            entry={entry()}
            onConfirm={handleReset}
            onCancel={() => setEntryToReset(null)}
          />
        )}
      </Show>

      <Show when={creatingBranchFor()}>
        {(entry) => (
          <div class="reset-modal-overlay" role="dialog" aria-modal="true">
            <div class="reset-modal-content">
              <div class="reset-modal-header">
                <h3 class="reset-modal-title">Create Branch from Reflog Entry</h3>
                <button
                  class="collapse-toggle"
                  onClick={() => setCreatingBranchFor(null)}
                  aria-label="Close"
                >
                  <Icon name="close" size={14} />
                </button>
              </div>

              <div class="reset-summary-card">
                <div class="reset-summary-row">
                  <span class="reset-summary-label">Target Commit:</span>
                  <span class="reset-summary-value font-mono">{entry().short_sha} ({entry().selector})</span>
                </div>
                <div class="reset-summary-row">
                  <span class="reset-summary-label">Action:</span>
                  <span class="reset-summary-value">{entry().operation}: {entry().message}</span>
                </div>
              </div>

              <div class="sync-form-group">
                <label for="new-reflog-branch-name" class="sync-form-label">New branch name:</label>
                <input
                  id="new-reflog-branch-name"
                  type="text"
                  class="reflog-input"
                  placeholder="e.g. recovery-branch"
                  value={newBranchName()}
                  onInput={(e) => setNewBranchName(e.currentTarget.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") void handleCreateBranch();
                  }}
                  autofocus
                />
              </div>

              <Show when={branchError()}>
                <div class="text-danger">{branchError()}</div>
              </Show>

              <div class="reset-modal-actions">
                <button
                  class="collapse-toggle"
                  onClick={() => setCreatingBranchFor(null)}
                >
                  Cancel
                </button>
                <button
                  onClick={() => void handleCreateBranch()}
                  disabled={!newBranchName().trim()}
                >
                  Create Branch
                </button>
              </div>
            </div>
          </div>
        )}
      </Show>
    </div>
  );
};
