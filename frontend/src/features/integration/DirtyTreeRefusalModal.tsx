import type { Component } from "solid-js";
import type { DirtyTreeDetails } from "../../api/types";
import { useModalEscape } from "../../layout/modal-escape";
import "./integration.css";

export interface DirtyTreeRefusalModalProps {
  operationName: string;
  details: DirtyTreeDetails;
  onStashAndProceed: () => void;
  onNavigateWorkingCopy: () => void;
  onClose?: () => void;
  onCancel?: () => void;
}

export const DirtyTreeRefusalModal: Component<DirtyTreeRefusalModalProps> = (props) => {
  const handleClose = () => {
    props.onClose?.();
    props.onCancel?.();
  };
  useModalEscape(handleClose);

  return (
    <div class="integration-modal-overlay">
      <div class="integration-modal-card">
        <div class="integration-modal-header">
          <h3>Uncommitted Changes Guard</h3>
          <button class="integration-btn" onClick={handleClose}>
            ✕
          </button>
        </div>
        <div class="integration-modal-body">
          <div class="integration-error-bar">
            <strong>Cannot start {props.operationName} with uncommitted modifications.</strong>
            <p style={{ margin: "var(--space-2) 0 0 0" }}>{props.details.summary}</p>
          </div>
          <p class="integration-banner-desc">
            GitTree prevents starting merge, rebase, cherry-pick, or revert operations on a dirty
            working copy to avoid unrecoverable loss or accidental merging of dirty files.
          </p>
        </div>
        <div class="integration-modal-footer">
          <button
            class="integration-btn integration-btn-primary"
            onClick={props.onStashAndProceed}
          >
            Stash & Proceed
          </button>
          <button class="integration-btn" onClick={props.onNavigateWorkingCopy}>
            Go to Working Copy
          </button>
          <button class="integration-btn" onClick={handleClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};
