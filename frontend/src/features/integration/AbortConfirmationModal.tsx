import type { Component } from "solid-js";
import { useModalEscape } from "../../layout/modal-escape";
import "./integration.css";

export interface AbortConfirmationModalProps {
  operationName: string;
  onConfirmAbort: () => void;
  onClose: () => void;
  loading?: boolean;
}

export const AbortConfirmationModal: Component<AbortConfirmationModalProps> = (props) => {
  useModalEscape(props.onClose);
  return (
    <div class="integration-modal-overlay">
      <div class="integration-modal-card">
        <div class="integration-modal-header">
          <h3>Abort {props.operationName}</h3>
          <button class="integration-btn" onClick={props.onClose}>
            ✕
          </button>
        </div>
        <div class="integration-modal-body">
          <div class="integration-error-bar">
            <strong>Are you sure you want to abort the current {props.operationName}?</strong>
          </div>
          <p class="integration-banner-desc">
            Aborting will restore HEAD, your current branch reference, and your working copy back to
            their exact pre-operation state. Any ongoing resolution of merge conflicts in your
            working tree will be discarded.
          </p>
        </div>
        <div class="integration-modal-footer">
          <button
            class="integration-btn integration-btn-danger"
            disabled={props.loading}
            onClick={props.onConfirmAbort}
          >
            {props.loading ? "Aborting..." : `Abort and Restore Prior State`}
          </button>
          <button class="integration-btn" disabled={props.loading} onClick={props.onClose}>
            Keep Resolving
          </button>
        </div>
      </div>
    </div>
  );
};
