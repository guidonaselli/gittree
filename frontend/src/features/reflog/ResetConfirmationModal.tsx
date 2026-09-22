import { type Component, createSignal, Show, onMount, onCleanup } from "solid-js";
import type { ReflogEntry, ResetMode } from "../../api/types";

export interface ResetConfirmationModalProps {
  targetRef: string;
  entry: ReflogEntry;
  onConfirm: (mode: ResetMode) => Promise<void>;
  onCancel: () => void;
}

export const ResetConfirmationModal: Component<ResetConfirmationModalProps> = (props) => {
  const [mode, setMode] = createSignal<ResetMode>("mixed");
  const [hardAcknowledged, setHardAcknowledged] = createSignal(false);
  const [isSubmitting, setIsSubmitting] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  onMount(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isSubmitting()) {
        props.onCancel();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    onCleanup(() => window.removeEventListener("keydown", onKeyDown));
  });

  const canSubmit = () => {
    if (isSubmitting()) return false;
    if (mode() === "hard") {
      return hardAcknowledged();
    }
    return true;
  };

  async function handleConfirm() {
    if (!canSubmit()) return;
    setIsSubmitting(true);
    setError(null);
    try {
      await props.onConfirm(mode());
    } catch (err) {
      setError(String(err));
      setIsSubmitting(false);
    }
  }

  return (
    <div class="reset-modal-overlay" role="dialog" aria-modal="true" aria-labelledby="reset-modal-title">
      <div class="reset-modal-content">
        <div class="reset-modal-header">
          <h2 id="reset-modal-title" class="reset-modal-title">
            Reset {props.targetRef} to Reflog Entry
          </h2>
          <button
            class="collapse-toggle"
            onClick={props.onCancel}
            disabled={isSubmitting()}
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        <div class="reset-summary-card">
          <div class="reset-summary-row">
            <span class="reset-summary-label">Target Ref:</span>
            <span class="reset-summary-value font-mono">{props.targetRef}</span>
          </div>
          <div class="reset-summary-row">
            <span class="reset-summary-label">Entry:</span>
            <span class="reset-summary-value font-mono">{props.entry.selector} ({props.entry.short_sha})</span>
          </div>
          <div class="reset-summary-row">
            <span class="reset-summary-label">Action:</span>
            <span class="reset-summary-value">{props.entry.operation}: {props.entry.message}</span>
          </div>
        </div>

        <div class="reset-radio-group">
          <label class="reset-radio-item">
            <input
              type="radio"
              name="reset-mode"
              value="mixed"
              checked={mode() === "mixed"}
              onChange={() => setMode("mixed")}
            />
            <div class="reset-radio-body">
              <span class="reset-radio-title">Mixed (Default / Recommended)</span>
              <span class="reset-radio-desc">
                Resets HEAD and the branch pointer. Preserves your working copy files and unstages changes.
              </span>
            </div>
          </label>

          <label class="reset-radio-item">
            <input
              type="radio"
              name="reset-mode"
              value="soft"
              checked={mode() === "soft"}
              onChange={() => setMode("soft")}
            />
            <div class="reset-radio-body">
              <span class="reset-radio-title">Soft</span>
              <span class="reset-radio-desc">
                Resets HEAD and the branch pointer only. Leaves all file modifications staged in the index.
              </span>
            </div>
          </label>

          <label class="reset-radio-item">
            <input
              type="radio"
              name="reset-mode"
              value="hard"
              checked={mode() === "hard"}
              onChange={() => setMode("hard")}
            />
            <div class="reset-radio-body">
              <span class="reset-radio-title text-danger">Hard (Destructive)</span>
              <span class="reset-radio-desc">
                Discards uncommitted changes! Resets HEAD, the branch pointer, index, and working directory.
              </span>
            </div>
          </label>
        </div>

        <Show when={mode() === "hard"}>
          <div class="reset-hard-warning">
            <div class="reset-hard-title">⚠️ Warning: Destructive Action</div>
            <div>
              Hard reset permanently overwrites files in your working directory to match the target commit.
              Any uncommitted modifications will be discarded and cannot be recovered by Git.
            </div>
            <label class="reset-ack-checkbox">
              <input
                type="checkbox"
                checked={hardAcknowledged()}
                onChange={(e) => setHardAcknowledged(e.currentTarget.checked)}
              />
              <span>I understand that uncommitted changes will be permanently discarded</span>
            </label>
          </div>
        </Show>

        <Show when={error()}>
          <div class="text-danger">{error()}</div>
        </Show>

        <div class="reset-modal-actions">
          <button
            class="collapse-toggle"
            onClick={props.onCancel}
            disabled={isSubmitting()}
          >
            Cancel
          </button>
          <button
            class={mode() === "hard" ? "reflog-action-btn-danger" : ""}
            onClick={() => void handleConfirm()}
            disabled={!canSubmit()}
          >
            {isSubmitting() ? "Resetting..." : `Reset ${props.targetRef} (${mode()})`}
          </button>
        </div>
      </div>
    </div>
  );
};
