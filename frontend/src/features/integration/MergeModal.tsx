import { type Component, Show, createSignal } from "solid-js";
import { createStash, startMerge } from "../../api/commands";
import type { DirtyTreeDetails, MergeOptions } from "../../api/types";
import { DirtyTreeRefusalModal } from "./DirtyTreeRefusalModal";
import "./integration.css";
import { Icon } from "../../ui/Icon";

export interface MergeModalProps {
  root: string;
  defaultTarget?: string;
  onClose: () => void;
  onSuccess: (newHead: string, isFastForward: boolean) => void;
  onConflict: (conflictingFiles: string[], message: string) => void;
  onNavigateWorkingCopy?: () => void;
}

export const MergeModal: Component<MergeModalProps> = (props) => {
  const [targetRef, setTargetRef] = createSignal(props.defaultTarget ?? "");
  const [noFf, setNoFf] = createSignal(false);
  const [ffOnly, setFfOnly] = createSignal(false);
  const [squash, setSquash] = createSignal(false);
  const [customMessage, setCustomMessage] = createSignal(
    props.defaultTarget ? `Merge branch '${props.defaultTarget}'` : ""
  );
  const [submitting, setSubmitting] = createSignal(false);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [dirtyDetails, setDirtyDetails] = createSignal<DirtyTreeDetails | null>(null);

  async function executeMerge() {
    if (!targetRef().trim()) {
      setErrorMessage("Please specify a target branch or ref to merge.");
      return;
    }

    setSubmitting(true);
    setErrorMessage(null);
    setDirtyDetails(null);

    const options: MergeOptions = {
      no_ff: noFf(),
      ff_only: ffOnly(),
      squash: squash(),
      message: customMessage().trim() ? customMessage().trim() : null,
    };

    try {
      const outcome = await startMerge(props.root, targetRef().trim(), options);
      if (outcome.status === "Success") {
        props.onSuccess(outcome.new_head, outcome.is_fast_forward);
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
        message: `Auto-stash before merge of ${targetRef()}`,
        include_untracked: true,
        keep_index: false,
      });
      await executeMerge();
    } catch (e) {
      setErrorMessage(String(e));
    }
  }

  return (
    <>
      <div class="integration-modal-overlay">
        <div class="integration-modal-card">
          <div class="integration-modal-header">
            <h3>Merge Branch or Ref into HEAD</h3>
            <button class="integration-btn" onClick={props.onClose} aria-label="Close">
              <Icon name="close" size={14} />
            </button>
          </div>

          <div class="integration-modal-body">
            <Show when={errorMessage()}>
              <div class="integration-error-bar">{errorMessage()}</div>
            </Show>

            <div class="integration-form-field">
              <label for="merge-target-input">Branch, tag, or commit to merge</label>
              <input
                id="merge-target-input"
                type="text"
                autofocus
                placeholder="e.g. feature/login or origin/main"
                value={targetRef()}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && targetRef().trim() && !submitting()) {
                    void executeMerge();
                  }
                }}
                onInput={(e) => {
                  setTargetRef(e.currentTarget.value);
                  if (!customMessage() || customMessage().startsWith("Merge branch")) {
                    setCustomMessage(`Merge branch '${e.currentTarget.value}'`);
                  }
                }}
              />
            </div>

            <div class="integration-form-field">
              <label class="integration-checkbox-label">
                <input
                  type="checkbox"
                  checked={noFf()}
                  disabled={ffOnly()}
                  onChange={(e) => setNoFf(e.currentTarget.checked)}
                />
                No fast-forward (--no-ff): always create a merge commit
              </label>

              <label class="integration-checkbox-label">
                <input
                  type="checkbox"
                  checked={ffOnly()}
                  disabled={noFf()}
                  onChange={(e) => setFfOnly(e.currentTarget.checked)}
                />
                Fast-forward only (--ff-only): refuse merge if non-fast-forward
              </label>

              <label class="integration-checkbox-label">
                <input
                  type="checkbox"
                  checked={squash()}
                  onChange={(e) => setSquash(e.currentTarget.checked)}
                />
                Squash commits (--squash): combine into working copy without merge commit
              </label>
            </div>

            <Show when={!ffOnly() && !squash()}>
              <div class="integration-form-field">
                <label for="merge-message-input">Commit message review</label>
                <textarea
                  id="merge-message-input"
                  rows={3}
                  value={customMessage()}
                  onInput={(e) => setCustomMessage(e.currentTarget.value)}
                  placeholder="Merge commit message..."
                />
              </div>
            </Show>
          </div>

          <div class="integration-modal-footer">
            <button
              class="integration-btn integration-btn-primary"
              disabled={submitting() || !targetRef().trim()}
              onClick={() => void executeMerge()}
            >
              {submitting() ? "Merging..." : "Merge into HEAD"}
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
            operationName="Merge"
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
