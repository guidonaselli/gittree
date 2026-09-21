import { type Component, Show, createSignal } from "solid-js";
import { createStash, startRevert } from "../../api/commands";
import type { DirtyTreeDetails, RevertOptions } from "../../api/types";
import { DirtyTreeRefusalModal } from "./DirtyTreeRefusalModal";
import "./integration.css";

export interface RevertModalProps {
  root: string;
  defaultCommit?: string;
  defaultSubject?: string;
  onClose: () => void;
  onSuccess: (newHead: string) => void;
  onConflict: (conflictingFiles: string[], message: string) => void;
  onNavigateWorkingCopy?: () => void;
}

export const RevertModal: Component<RevertModalProps> = (props) => {
  const [commitRef, setCommitRef] = createSignal(props.defaultCommit ?? "");
  const [noCommit, setNoCommit] = createSignal(false);
  const [submitting, setSubmitting] = createSignal(false);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [dirtyDetails, setDirtyDetails] = createSignal<DirtyTreeDetails | null>(null);

  async function executeRevert() {
    if (!commitRef().trim()) {
      setErrorMessage("Please specify a commit sha to revert.");
      return;
    }

    setSubmitting(true);
    setErrorMessage(null);
    setDirtyDetails(null);

    const options: RevertOptions = {
      no_commit: noCommit(),
    };

    try {
      const outcome = await startRevert(props.root, commitRef().trim(), options);
      if (outcome.status === "Success") {
        props.onSuccess(outcome.new_head);
      } else if (outcome.status === "Conflict") {
        props.onConflict(outcome.conflicting_files, outcome.message);
      } else if (outcome.status === "DirtyTreeRefusal") {
        setDirtyDetails(outcome);
      }
    } catch (e) {
      setErrorMessage(String(e));
    } finally {
      setSubmitting(false);
    }
  }

  async function handleStashAndProceed() {
    setDirtyDetails(null);
    try {
      await createStash(props.root, {
        message: `Auto-stash before revert of ${commitRef()}`,
        include_untracked: true,
        keep_index: false,
      });
      await executeRevert();
    } catch (e) {
      setErrorMessage(String(e));
    }
  }

  return (
    <>
      <div class="integration-modal-overlay">
        <div class="integration-modal-card">
          <div class="integration-modal-header">
            <h3>Revert Commit</h3>
            <button class="integration-btn" onClick={props.onClose}>
              ✕
            </button>
          </div>

          <div class="integration-modal-body">
            <Show when={errorMessage()}>
              <div class="integration-error-bar">{errorMessage()}</div>
            </Show>

            <Show when={props.defaultSubject}>
              <div class="integration-banner-desc">
                Inverting: <strong>{props.defaultSubject}</strong>
              </div>
            </Show>

            <div class="integration-form-field">
              <label for="revert-target-input">Commit SHA or reference to revert</label>
              <input
                id="revert-target-input"
                type="text"
                autofocus
                placeholder="e.g. 7f82bb5"
                value={commitRef()}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && commitRef().trim() && !submitting()) {
                    void executeRevert();
                  }
                }}
                onInput={(e) => setCommitRef(e.currentTarget.value)}
              />
            </div>

            <div class="integration-form-field">
              <label class="integration-checkbox-label">
                <input
                  type="checkbox"
                  checked={noCommit()}
                  onChange={(e) => setNoCommit(e.currentTarget.checked)}
                />
                Apply inverse changes to index without creating a commit (-n / --no-commit)
              </label>
            </div>
          </div>

          <div class="integration-modal-footer">
            <button
              class="integration-btn integration-btn-primary"
              disabled={submitting() || !commitRef().trim()}
              onClick={() => void executeRevert()}
            >
              {submitting() ? "Reverting..." : "Revert Commit"}
            </button>
            <button class="integration-btn" disabled={submitting()} onClick={props.onClose}>
              Cancel
            </button>
          </div>
        </div>
      </div>

      <Show when={dirtyDetails()}>
        {(details) => (
          <DirtyTreeRefusalModal
            operationName="Revert"
            details={details()}
            onStashAndProceed={() => void handleStashAndProceed()}
            onNavigateWorkingCopy={() => {
              props.onClose();
              props.onNavigateWorkingCopy?.();
            }}
            onClose={() => setDirtyDetails(null)}
          />
        )}
      </Show>
    </>
  );
};
