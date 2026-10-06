import { type Component } from "solid-js";
import "./sync.css";
import { Icon } from "../../ui/Icon";

export interface PushRejectedModalProps {
  remoteMessage: string;
  onPullThenRetry: () => void;
  onClose: () => void;
}

export const PushRejectedModal: Component<PushRejectedModalProps> = (props) => {
  return (
    <div class="sync-modal-overlay" role="dialog" aria-modal="true" aria-labelledby="push-rejected-title">
      <div class="sync-modal-content">
        <div class="sync-modal-header">
          <h2 id="push-rejected-title" class="sync-modal-title text-danger">
            Push Rejected (Non-Fast-Forward)
          </h2>
          <button class="collapse-toggle" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>

        <div class="sync-danger-alert">
          <div class="sync-danger-title">Remote contains commits not present locally</div>
          <div>
            The remote repository contains changes that you do not have in your local branch.
            GitTree never forces pushes implicitly. Please pull the remote changes first, then retry pushing.
          </div>
        </div>

        <div class="sync-form-group">
          <span class="sync-form-label">Remote message (verbatim):</span>
          <div class="sync-verbatim-error">{props.remoteMessage}</div>
        </div>

        <div class="sync-modal-actions">
          <button class="collapse-toggle" onClick={props.onClose}>
            Cancel
          </button>
          <button class="button-primary" onClick={props.onPullThenRetry}>
            Pull then Retry
          </button>
        </div>
      </div>
    </div>
  );
};
