import { type Component, For, Show, createSignal } from "solid-js";
import { abortOperation, continueOperation, skipOperation } from "../../api/commands";
import type { AbortOutcome, ActiveOperationDetail } from "../../api/types";
import { AbortConfirmationModal } from "./AbortConfirmationModal";
import "./integration.css";

export interface ActiveOperationBannerProps {
  root: string;
  operation: ActiveOperationDetail;
  onOperationUpdated: () => void;
  onOperationAborted: (outcome: AbortOutcome) => void;
  onNavigateWorkingCopy?: () => void;
}

export const ActiveOperationBanner: Component<ActiveOperationBannerProps> = (props) => {
  const [showAbortModal, setShowAbortModal] = createSignal(false);
  const [loading, setLoading] = createSignal(false);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);

  async function handleContinue() {
    setLoading(true);
    setErrorMessage(null);
    try {
      const res = await continueOperation(props.root);
      if (res.status === "Failed") {
        setErrorMessage(res.message);
      } else {
        props.onOperationUpdated();
      }
    } catch (e) {
      setErrorMessage(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleSkip() {
    setLoading(true);
    setErrorMessage(null);
    try {
      const res = await skipOperation(props.root);
      if (res.status === "Failed") {
        setErrorMessage(res.message);
      } else {
        props.onOperationUpdated();
      }
    } catch (e) {
      setErrorMessage(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleConfirmAbort() {
    setLoading(true);
    setErrorMessage(null);
    try {
      const outcome = await abortOperation(props.root);
      setShowAbortModal(false);
      props.onOperationAborted(outcome);
    } catch (e) {
      setErrorMessage(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <>
      <div class="integration-active-banner">
        <div class="integration-banner-header">
          <div class="integration-banner-title-group">
            <span class="integration-banner-badge">{props.operation.kind}</span>
            <span class="integration-banner-title">{props.operation.title}</span>
          </div>
          <div class="integration-banner-actions">
            <button
              class="integration-btn integration-btn-primary"
              disabled={loading()}
              onClick={() => void handleContinue()}
            >
              {loading() ? "Processing..." : "Continue"}
            </button>
            <Show when={props.operation.can_skip}>
              <button
                class="integration-btn"
                disabled={loading()}
                onClick={() => void handleSkip()}
              >
                Skip
              </button>
            </Show>
            <button
              class="integration-btn integration-btn-danger"
              disabled={loading()}
              onClick={() => setShowAbortModal(true)}
            >
              Abort...
            </button>
          </div>
        </div>

        <div class="integration-banner-desc">{props.operation.description}</div>

        <Show when={props.operation.conflicting_files.length > 0}>
          <div class="integration-conflicts-row">
            <span class="integration-conflicts-label">
              Conflicting files ({props.operation.conflicting_files.length}):
            </span>
            <For each={props.operation.conflicting_files}>
              {(file) => (
                <span
                  class="integration-conflict-pill"
                  title="Click to view in working copy"
                  onClick={() => props.onNavigateWorkingCopy?.()}
                  style={{ cursor: "pointer" }}
                >
                  {file}
                </span>
              )}
            </For>
          </div>
        </Show>

        <Show when={errorMessage()}>
          <div class="integration-error-bar">{errorMessage()}</div>
        </Show>
      </div>

      <Show when={showAbortModal()}>
        <AbortConfirmationModal
          operationName={props.operation.title}
          loading={loading()}
          onConfirmAbort={() => void handleConfirmAbort()}
          onClose={() => setShowAbortModal(false)}
        />
      </Show>
    </>
  );
};
