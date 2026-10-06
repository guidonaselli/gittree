import {
  type Component,
  For,
  Show,
  createEffect,
  createResource,
  createSignal,
} from "solid-js";
import {
  applyStash,
  clearStashes,
  createStash,
  dropStash,
  getStashes,
  inspectStash,
  popStash,
} from "../../api/commands";
import type { StashEntry } from "../../api/types";
import "./stashes.css";
import { Icon } from "../../ui/Icon";

export interface StashesViewProps {
  root: string;
  onNavigateWorkingCopy?: () => void;
}

export const StashesView: Component<StashesViewProps> = (props) => {
  const [stashes, { refetch: refetchStashes }] = createResource(
    () => props.root,
    (root) => getStashes(root)
  );

  const [selectedSelector, setSelectedSelector] = createSignal<string | null>(null);
  const [reinstateIndex, setReinstateIndex] = createSignal(false);
  const [showCreateModal, setShowCreateModal] = createSignal(false);
  const [dropTarget, setDropTarget] = createSignal<StashEntry | null>(null);
  const [showClearConfirm, setShowClearConfirm] = createSignal(false);

  const [actionNotice, setActionNotice] = createSignal<string | null>(null);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [conflictOutcome, setConflictOutcome] = createSignal<{
    selector: string;
    conflicting_files: string[];
    message: string;
    stash_retained: boolean;
  } | null>(null);

  // Auto-select first stash when stashes load if none selected
  createEffect(() => {
    const list = stashes();
    if (list && list.length > 0) {
      if (!selectedSelector() || !list.some((s) => s.selector === selectedSelector())) {
        setSelectedSelector(list[0].selector);
      }
    } else {
      setSelectedSelector(null);
    }
  });

  const [detailResource] = createResource(
    () => {
      const sel = selectedSelector();
      const root = props.root;
      return sel ? { root, sel } : null;
    },
    async ({ root, sel }) => {
      try {
        return await inspectStash(root, sel);
      } catch (e) {
        setErrorMessage(String(e));
        return null;
      }
    }
  );

  async function handleApply(selector: string, isPop: boolean) {
    setErrorMessage(null);
    try {
      const outcome = isPop
        ? await popStash(props.root, selector, reinstateIndex())
        : await applyStash(props.root, selector, reinstateIndex());

      if (outcome.kind === "Clean") {
        setActionNotice(
          isPop
            ? `Stash '${selector}' popped and applied successfully.`
            : `Stash '${selector}' applied successfully.`
        );
        setTimeout(() => setActionNotice(null), 5000);
        refetchStashes();
      } else {
        setConflictOutcome({
          selector,
          conflicting_files: outcome.conflicting_files,
          message: outcome.message,
          stash_retained: outcome.stash_retained,
        });
        refetchStashes();
      }
    } catch (e) {
      setErrorMessage(String(e));
    }
  }

  return (
    <div class="stashes-view">
      {/* Left sidebar: Stashes List */}
      <div class="stashes-sidebar">
        <div class="stashes-toolbar">
          <button class="stashes-btn stashes-btn-primary" onClick={() => setShowCreateModal(true)}>
            New stash...
          </button>
          <button class="stashes-btn" onClick={() => refetchStashes()}>
            Refresh
          </button>
          <button
            class="stashes-btn stashes-btn-danger"
            disabled={!stashes() || stashes()!.length === 0}
            onClick={() => setShowClearConfirm(true)}
            title="Drop all stashes"
          >
            Clear all
          </button>
        </div>

        <div class="stashes-list-container">
          <Show when={stashes.loading}>
            <p class="text-muted" style={{ padding: "var(--space-3)" }}>
              Loading stashes...
            </p>
          </Show>

          <Show when={!stashes.loading && (!stashes() || stashes()!.length === 0)}>
            <p class="text-muted" style={{ padding: "var(--space-3)" }}>
              No stashes saved in this repository.
            </p>
          </Show>

          <ul class="stashes-list">
            <For each={stashes()}>
              {(stash) => {
                const isSelected = () => selectedSelector() === stash.selector;
                return (
                  <li
                    class={`stash-list-item ${isSelected() ? "stash-list-item-selected" : ""}`}
                    onClick={() => setSelectedSelector(stash.selector)}
                  >
                    <div class="stash-list-item-top">
                      <span class="stash-selector">{stash.selector}</span>
                      <Show when={stash.branch}>
                        <span class="stash-branch-badge">{stash.branch}</span>
                      </Show>
                    </div>
                    <div class="stash-message" title={stash.message}>
                      {stash.message}
                    </div>
                    <div class="stash-date">{stash.date}</div>
                  </li>
                );
              }}
            </For>
          </ul>
        </div>
      </div>

      {/* Right panel: Inspection & Diff Before Applying */}
      <div class="stashes-detail-panel">
        <Show when={errorMessage()}>
          <div class="stash-banner-error">{errorMessage()}</div>
        </Show>

        <Show when={actionNotice()}>
          <div class="stash-banner-notice">{actionNotice()}</div>
        </Show>

        <Show
          when={detailResource()}
          fallback={
            <div class="text-muted" style={{ padding: "var(--space-4)" }}>
              {stashes() && stashes()!.length > 0
                ? "Select a stash to inspect its changes before applying."
                : "Create a stash or use stash and checkout to preserve temporary work."}
            </div>
          }
        >
          {(detail) => (
            <>
              <div class="stash-detail-header">
                <div class="stash-detail-info">
                  <div class="stash-detail-title">
                    <span>{detail().entry.selector}</span>
                    <Show when={detail().entry.branch}>
                      <span class="stash-branch-badge">{detail().entry.branch}</span>
                    </Show>
                  </div>
                  <div class="stash-detail-meta">
                    <span>Commit: <code>{detail().entry.short_sha}</code></span>
                    <span>Date: {detail().entry.date}</span>
                  </div>
                  <div class="stash-message" style={{ "margin-top": "var(--space-1)" }}>
                    {detail().entry.message}
                  </div>
                </div>

                <div class="stash-detail-actions">
                  <label class="stash-checkbox-label">
                    <input
                      type="checkbox"
                      checked={reinstateIndex()}
                      onChange={(e) => setReinstateIndex(e.currentTarget.checked)}
                    />
                    <span>Reinstate index (--index)</span>
                  </label>
                  <button
                    class="stashes-btn stashes-btn-primary"
                    onClick={() => void handleApply(detail().entry.selector, false)}
                    title="Apply stash changes into working directory, keeping stash"
                  >
                    Apply Stash
                  </button>
                  <button
                    class="stashes-btn"
                    onClick={() => void handleApply(detail().entry.selector, true)}
                    title="Apply stash changes and drop it from the list"
                  >
                    Pop Stash
                  </button>
                  <button
                    class="stashes-btn stashes-btn-danger"
                    onClick={() => setDropTarget(detail().entry)}
                    title="Delete this stash without applying"
                  >
                    Drop
                  </button>
                </div>
              </div>

              {/* Changed files summary */}
              <div class="stash-files-summary">
                <div class="stash-section-title">
                  Files Changed ({detail().changed_files.length + detail().untracked_files.length})
                </div>
                <ul class="stash-files-list">
                  <For each={detail().changed_files}>
                    {(file) => (
                      <li class="stash-file-row">
                        <span>{file.path}</span>
                        <div>
                          <span class="stash-stat-badge-add">+{file.additions}</span>
                          <span class="stash-stat-badge-del">-{file.deletions}</span>
                        </div>
                      </li>
                    )}
                  </For>
                  <For each={detail().untracked_files}>
                    {(file) => (
                      <li class="stash-file-row">
                        <span>{file}</span>
                        <span class="stash-untracked-badge">untracked</span>
                      </li>
                    )}
                  </For>
                </ul>
              </div>

              {/* Full Unified Diff Viewer */}
              <div class="stash-diff-container">
                <div class="stash-section-title">Unified Diff</div>
                <div class="stash-diff-viewer">
                  {detail().diff || "(No diff contents)"}
                </div>
              </div>
            </>
          )}
        </Show>
      </div>

      {/* Create Stash Modal */}
      <Show when={showCreateModal()}>
        <CreateStashDialog
          root={props.root}
          onClose={() => setShowCreateModal(false)}
          onSuccess={() => {
            setShowCreateModal(false);
            refetchStashes();
            setActionNotice("Stash created successfully.");
            setTimeout(() => setActionNotice(null), 5000);
          }}
        />
      </Show>

      {/* Drop Stash Confirmation Dialog */}
      <Show when={dropTarget()}>
        {(target) => (
          <DropStashDialog
            root={props.root}
            stash={target()}
            onClose={() => setDropTarget(null)}
            onSuccess={() => {
              const sel = target().selector;
              setDropTarget(null);
              refetchStashes();
              setActionNotice(`Stash '${sel}' dropped.`);
              setTimeout(() => setActionNotice(null), 5000);
            }}
          />
        )}
      </Show>

      {/* Clear All Confirmation Dialog */}
      <Show when={showClearConfirm()}>
        <ClearStashesDialog
          root={props.root}
          onClose={() => setShowClearConfirm(false)}
          onSuccess={() => {
            setShowClearConfirm(false);
            refetchStashes();
            setActionNotice("All stashes cleared.");
            setTimeout(() => setActionNotice(null), 5000);
          }}
        />
      </Show>

      {/* Conflict on Apply / Pop Modal */}
      <Show when={conflictOutcome()}>
        {(conflict) => (
          <div class="stash-modal-overlay">
            <div class="stash-modal-card">
              <div class="stash-modal-header">
                <h3>Conflicts During Stash Operation</h3>
                <button class="stashes-btn" onClick={() => setConflictOutcome(null)} aria-label="Close">
                  <Icon name="close" size={14} />
                </button>
              </div>
              <div class="stash-modal-body">
                <div class="stash-banner-conflict">
                  <strong>
                    Git encountered merge conflicts while applying {conflict().selector}.
                  </strong>
                  <p>
                    Your uncommitted changes or branch state conflict with the stashed modifications.
                  </p>
                </div>

                <Show when={conflict().stash_retained}>
                  <div class="stash-banner-notice">
                    <strong>Stash Retained:</strong> Your stash entry{" "}
                    <code>{conflict().selector}</code> has been preserved in your stash list and
                    was not lost.
                  </div>
                </Show>

                <div class="stash-section-title">Conflicting Files:</div>
                <ul class="stash-files-list">
                  <For
                    each={conflict().conflicting_files}
                    fallback={<li class="text-muted">No specific files parsed. See details below.</li>}
                  >
                    {(file) => (
                      <li class="stash-file-row">
                        <strong style={{ color: "var(--color-warning)" }}>{file}</strong>
                      </li>
                    )}
                  </For>
                </ul>

                <div class="stash-conflict-box">{conflict().message}</div>
              </div>
              <div class="stash-modal-footer">
                <button
                  class="stashes-btn stashes-btn-primary"
                  onClick={() => {
                    setConflictOutcome(null);
                    props.onNavigateWorkingCopy?.();
                  }}
                >
                  Go to Working Copy
                </button>
                <button class="stashes-btn" onClick={() => setConflictOutcome(null)}>
                  Close
                </button>
              </div>
            </div>
          </div>
        )}
      </Show>
    </div>
  );
};

interface CreateStashDialogProps {
  root: string;
  onClose: () => void;
  onSuccess: () => void;
}

const CreateStashDialog: Component<CreateStashDialogProps> = (props) => {
  const [message, setMessage] = createSignal("");
  const [includeUntracked, setIncludeUntracked] = createSignal(true);
  const [keepIndex, setKeepIndex] = createSignal(false);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function handleCreate() {
    setLoading(true);
    setError(null);
    try {
      await createStash(props.root, {
        message: message().trim() || null,
        include_untracked: includeUntracked(),
        keep_index: keepIndex(),
      });
      props.onSuccess();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="stash-modal-overlay">
      <div class="stash-modal-card">
        <div class="stash-modal-header">
          <h3>Create Stash</h3>
          <button class="stashes-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="stash-modal-body">
          <Show when={error()}>
            <div class="stash-banner-error">{error()}</div>
          </Show>

          <div class="stash-modal-field">
            <label for="stash-message-input">Message (optional)</label>
            <input
              id="stash-message-input"
              type="text"
              placeholder="e.g. WIP for feature XYZ"
              value={message()}
              onInput={(e) => setMessage(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleCreate();
              }}
            />
          </div>

          <label class="stash-checkbox-label">
            <input
              type="checkbox"
              checked={includeUntracked()}
              onChange={(e) => setIncludeUntracked(e.currentTarget.checked)}
            />
            <span>Include untracked files (-u)</span>
          </label>

          <label class="stash-checkbox-label">
            <input
              type="checkbox"
              checked={keepIndex()}
              onChange={(e) => setKeepIndex(e.currentTarget.checked)}
            />
            <span>Keep staged index (--keep-index)</span>
          </label>
        </div>
        <div class="stash-modal-footer">
          <button
            class="stashes-btn stashes-btn-primary"
            disabled={loading()}
            onClick={() => void handleCreate()}
          >
            {loading() ? "Stashing..." : "Create Stash"}
          </button>
          <button class="stashes-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

interface DropStashDialogProps {
  root: string;
  stash: StashEntry;
  onClose: () => void;
  onSuccess: () => void;
}

const DropStashDialog: Component<DropStashDialogProps> = (props) => {
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function handleDrop() {
    setLoading(true);
    setError(null);
    try {
      await dropStash(props.root, props.stash.selector);
      props.onSuccess();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="stash-modal-overlay">
      <div class="stash-modal-card">
        <div class="stash-modal-header">
          <h3>Drop Stash</h3>
          <button class="stashes-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="stash-modal-body">
          <Show when={error()}>
            <div class="stash-banner-error">{error()}</div>
          </Show>

          <p>
            Are you sure you want to permanently drop stash{" "}
            <strong style={{ color: "var(--color-danger)" }}>{props.stash.selector}</strong>?
          </p>
          <p class="text-muted">
            Message: <em>{props.stash.message}</em>
          </p>
          <p class="text-muted">
            This action cannot be undone unless recovered immediately via the reflog.
          </p>
        </div>
        <div class="stash-modal-footer">
          <button
            class="stashes-btn stashes-btn-danger"
            disabled={loading()}
            onClick={() => void handleDrop()}
          >
            {loading() ? "Dropping..." : "Drop Stash"}
          </button>
          <button class="stashes-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

interface ClearStashesDialogProps {
  root: string;
  onClose: () => void;
  onSuccess: () => void;
}

const ClearStashesDialog: Component<ClearStashesDialogProps> = (props) => {
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function handleClear() {
    setLoading(true);
    setError(null);
    try {
      await clearStashes(props.root);
      props.onSuccess();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="stash-modal-overlay">
      <div class="stash-modal-card">
        <div class="stash-modal-header">
          <h3>Clear All Stashes</h3>
          <button class="stashes-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="stash-modal-body">
          <Show when={error()}>
            <div class="stash-banner-error">{error()}</div>
          </Show>

          <p>
            Are you sure you want to delete <strong style={{ color: "var(--color-danger)" }}>ALL stashes</strong> in this repository?
          </p>
          <p class="text-muted">
            All stashed changes across all branches will be dropped.
          </p>
        </div>
        <div class="stash-modal-footer">
          <button
            class="stashes-btn stashes-btn-danger"
            disabled={loading()}
            onClick={() => void handleClear()}
          >
            {loading() ? "Clearing all..." : "Clear All Stashes"}
          </button>
          <button class="stashes-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};
