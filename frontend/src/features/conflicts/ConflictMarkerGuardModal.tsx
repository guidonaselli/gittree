import { createEffect, onCleanup, type Component, For } from "solid-js";
import "./conflicts.css";
import { Icon } from "../../ui/Icon";

export interface ConflictMarkerGuardModalProps {
  file: string;
  markerLines: number[];
  previewLines: string[];
  onConfirmOverride: () => void;
  onCancel: () => void;
}

export const ConflictMarkerGuardModal: Component<ConflictMarkerGuardModalProps> = (props) => {
  let cancelBtnRef: HTMLButtonElement | undefined;

  createEffect(() => {
    cancelBtnRef?.focus();

    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        props.onCancel();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    onCleanup(() => window.removeEventListener("keydown", handleKeyDown));
  });

  return (
    <div class="conflict-guard-overlay" role="dialog" aria-modal="true" aria-labelledby="marker-guard-title">
      <div class="conflict-guard-card">
        <div class="conflict-guard-header">
          <h3 id="marker-guard-title">Unresolved Conflict Markers</h3>
          <button class="conflict-btn" onClick={props.onCancel} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="conflict-guard-body">
          <div class="conflict-guard-warning">
            <strong>Warning:</strong> File <code>{props.file}</code> still contains unresolved conflict markers on line{props.markerLines.length > 1 ? "s" : ""} <strong>{props.markerLines.join(", ")}</strong>. Staging this file will record raw conflict markers into your commit history.
          </div>
          <div>
            <span class="candidate-label">Detected marker lines:</span>
            <div class="conflict-guard-preview-box">
              <For each={props.previewLines}>
                {(line) => <div class="conflict-guard-preview-line">{line}</div>}
              </For>
            </div>
          </div>
          <p style={{ "font-size": "var(--font-size-sm)", color: "var(--color-text-muted)", margin: "0" }}>
            If you intended to preserve these markers or have already reviewed them, you may explicitly override this protection to proceed with staging.
          </p>
        </div>
        <div class="conflict-guard-footer">
          <button
            ref={cancelBtnRef}
            class="conflict-btn"
            onClick={props.onCancel}
          >
            Cancel
          </button>
          <button
            class="conflict-btn conflict-btn-danger"
            onClick={props.onConfirmOverride}
          >
            Override & Stage
          </button>
        </div>
      </div>
    </div>
  );
};
