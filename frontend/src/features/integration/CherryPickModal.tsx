import { type Component, Show, createSignal } from "solid-js";
import { createStash, startCherryPick } from "../../api/commands";
import type { CherryPickOptions, DirtyTreeDetails } from "../../api/types";
import { DirtyTreeRefusalModal } from "./DirtyTreeRefusalModal";
import "./integration.css";
import { Icon } from "../../ui/Icon";

export interface CherryPickModalProps {
  root: string;
  defaultCommit?: string;
  defaultSubject?: string;
  onClose: () => void;
  onSuccess: (newHead: string) => void;
  onConflict: (conflictingFiles: string[], message: string) => void;
  onNavigateWorkingCopy?: () => void;
}

export const CherryPickModal: Component<CherryPickModalProps> = (props) => {
  const [commitRef, setCommitRef] = createSignal(props.defaultCommit ?? "");
  const [noCommit, setNoCommit] = createSignal(false);
  const [signoff, setSignoff] = createSignal(false);
  const [submitting, setSubmitting] = createSignal(false);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [dirtyDetails, setDirtyDetails] = createSignal<DirtyTreeDetails | null>(null);

  async function executeCherryPick() {
    if (!commitRef().trim()) {
      setErrorMessage("Please specify a commit sha to cherry-pick.");
      return;
    }

    setSubmitting(true);
    setErrorMessage(null);
    setDirtyDetails(null);

    const options: CherryPickOptions = {
      no_commit: noCommit(),
      signoff: signoff(),
    };

    try {
      const outcome = await startCherryPick(props.root, commitRef().trim(), options);
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
        message: `Auto-stash before cherry-pick of ${commitRef()}`,
        include_untracked: true,
        keep_index: false,
      });
      await executeCherryPick();
    } catch (e) {
      setErrorMessage(String(e));
    }
  }

  return (
    <>
      <div class="integration-modal-overlay">
        <div class="integration-modal-card">
          <div class="integration-modal-header">
            <h3>Cherry-Pick Commit</h3>
            <button class="integration-btn" onClick={props.onClose} aria-label="Close">
              <Icon name="close" size={14} />
            </button>
          </div>

          <div class="integration-modal-body">
            <Show when={errorMessage()}>
              <div class="integration-error-bar">{errorMessage()}</div>
            </Show>

            <Show when={props.defaultSubject}>
              <div class="integration-banner-desc">
                Applying: <strong>{props.defaultSubject}</strong>
              </div>
            </Show>

            <div class="integration-form-field">
              <label for="cherry-pick-commit-input">Commit SHA to cherry-pick</label>
              <input
                id="cherry-pick-commit-input"
                type="text"
                autofocus
                placeholder="Full or short SHA (e.g. a1b2c3d)"
                value={commitRef()}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && commitRef().trim() && !submitting()) {
                    void executeCherryPick();
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
                Apply changes to index without creating a commit (-n / --no-commit)
              </label>

              <label class="integration-checkbox-label">
                <input
                  type="checkbox"
                  checked={signoff()}
                  onChange={(e) => setSignoff(e.currentTarget.checked)}
                />
                Add Signed-off-by line (-s / --signoff)
              </label>
            </div>
          </div>

          <div class="integration-modal-footer">
            <button
              class="integration-btn integration-btn-primary"
              disabled={submitting() || !commitRef().trim()}
              onClick={() => void executeCherryPick()}
            >
              {submitting() ? "Cherry-picking..." : "Cherry-pick into HEAD"}
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
            operationName="Cherry-Pick"
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
