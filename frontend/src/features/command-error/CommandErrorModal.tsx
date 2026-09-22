import {
  type Component,
  createEffect,
  createSignal,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import "./command-error.css";

export interface CommandErrorModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  command?: string;
  exitStatus?: number;
  stderr: string;
}

export const CommandErrorModal: Component<CommandErrorModalProps> = (props) => {
  const [copied, setCopied] = createSignal(false);
  let previousActiveElement: HTMLElement | null = null;

  createEffect(() => {
    if (props.open) {
      previousActiveElement = document.activeElement as HTMLElement | null;
      setCopied(false);
    } else {
      if (previousActiveElement && typeof previousActiveElement.focus === "function") {
        previousActiveElement.focus();
      }
    }
  });

  onMount(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (props.open && e.key === "Escape") {
        e.preventDefault();
        props.onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    onCleanup(() => window.removeEventListener("keydown", onKeyDown, true));
  });

  async function copyDetails() {
    const details = [
      props.command ? `Command: ${props.command}` : "",
      props.exitStatus !== undefined ? `Exit Status: ${props.exitStatus}` : "",
      `Standard Error:\n${props.stderr}`,
    ]
      .filter(Boolean)
      .join("\n\n");

    try {
      await navigator.clipboard.writeText(details);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard write failure tolerated
    }
  }

  return (
    <Show when={props.open}>
      <div
        class="command-error-backdrop"
        onClick={(e) => {
          if (e.target === e.currentTarget) props.onClose();
        }}
        role="presentation"
      >
        <div
          class="command-error-modal"
          role="alertdialog"
          aria-modal="true"
          aria-label={props.title ?? "Command Execution Error"}
        >
          <div class="command-error-header">
            <h2>
              <span>⚠</span>
              <span>{props.title ?? "Command Execution Failed"}</span>
            </h2>
            <button
              type="button"
              class="command-error-close-btn"
              onClick={props.onClose}
              title="Close (Esc)"
              aria-label="Close"
            >
              ✕
            </button>
          </div>

          <div class="command-error-body">
            <Show when={props.command}>
              <div class="command-error-field-label">Command Line (Verbatim)</div>
              <pre class="command-error-code-block">{props.command}</pre>
            </Show>

            <Show when={props.exitStatus !== undefined}>
              <div class="command-error-field-label">Exit Status</div>
              <div class="command-error-code-block">
                {props.exitStatus} (Non-zero exit)
              </div>
            </Show>

            <div class="command-error-field-label">Standard Error (Verbatim)</div>
            <pre class="command-error-code-block command-error-stderr-block">
              {props.stderr}
            </pre>
          </div>

          <div class="command-error-footer">
            <button
              type="button"
              class="command-error-copy-btn"
              onClick={copyDetails}
            >
              {copied() ? "✓ Copied" : "Copy Details"}
            </button>
            <button
              type="button"
              class="command-error-dismiss-btn"
              onClick={props.onClose}
            >
              Dismiss
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
};
