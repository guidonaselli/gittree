import { createSignal, onMount, Show, type Component } from "solid-js";
import type { AskpassPromptPayload } from "../../api/types";
import { useModalEscape } from "../../layout/modal-escape";
import "./sync.css";

export interface AskpassModalProps {
  prompt: AskpassPromptPayload;
  onSubmit: (id: string, response: string) => void;
  onCancel: (id: string) => void;
}

export const AskpassModal: Component<AskpassModalProps> = (props) => {
  useModalEscape(() => props.onCancel(props.prompt.id));
  const [response, setResponse] = createSignal("");
  const [showSecret, setShowSecret] = createSignal(false);
  let inputEl: HTMLInputElement | undefined;

  onMount(() => {
    inputEl?.focus();
  });

  const getTitle = () => {
    switch (props.prompt.prompt_type) {
      case "host_key":
        return "SSH Host Key Verification";
      case "passphrase":
        return "SSH Key Passphrase";
      case "two_factor":
        return "Two-Factor Authentication (2FA)";
      case "username":
        return "Git Authentication (Username)";
      case "password":
      default:
        return "Git Authentication (Password / Token)";
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      props.onSubmit(props.prompt.id, response());
    } else if (e.key === "Escape") {
      e.preventDefault();
      props.onCancel(props.prompt.id);
    }
  };

  return (
    <div class="sync-modal-overlay" role="dialog" aria-modal="true" aria-labelledby="askpass-modal-title">
      <div class="sync-modal-content" onKeyDown={handleKeyDown}>
        <div class="sync-modal-header">
          <h2 id="askpass-modal-title" class="sync-modal-title">
            {getTitle()}
          </h2>
          <button class="collapse-toggle" onClick={() => props.onCancel(props.prompt.id)} aria-label="Cancel">
            ✕
          </button>
        </div>

        <div class="sync-form-group">
          <span class="sync-form-label">Authentication Request:</span>
          <div class="sync-verbatim-error">{props.prompt.prompt}</div>
        </div>

        <Show
          when={props.prompt.prompt_type !== "host_key"}
          fallback={
            <div class="sync-modal-actions">
              <button class="collapse-toggle text-danger" onClick={() => props.onSubmit(props.prompt.id, "no")}>
                No (Reject)
              </button>
              <button class="button-primary" onClick={() => props.onSubmit(props.prompt.id, "yes")}>
                Yes (Accept & Connect)
              </button>
            </div>
          }
        >
          <div class="sync-form-group">
            <label class="sync-form-label" for="askpass-input">
              {props.prompt.prompt_type === "username"
                ? "Username"
                : props.prompt.prompt_type === "two_factor"
                ? "Verification Code"
                : "Passphrase / Secret"}
            </label>
            <div style={{ display: "flex", gap: "var(--space-2)" }}>
              <input
                id="askpass-input"
                ref={inputEl}
                class="sync-form-input"
                style={{ flex: 1 }}
                type={
                  props.prompt.prompt_type === "username" ||
                  props.prompt.prompt_type === "two_factor" ||
                  showSecret()
                    ? "text"
                    : "password"
                }
                value={response()}
                onInput={(e) => setResponse(e.currentTarget.value)}
              />
              <Show
                when={
                  props.prompt.prompt_type !== "username" &&
                  props.prompt.prompt_type !== "two_factor"
                }
              >
                <button
                  type="button"
                  class="collapse-toggle"
                  onClick={() => setShowSecret(!showSecret())}
                  aria-label={showSecret() ? "Hide password" : "Show password"}
                >
                  {showSecret() ? "Hide" : "Show"}
                </button>
              </Show>
            </div>
          </div>

          <div class="sync-modal-actions">
            <button class="collapse-toggle" onClick={() => props.onCancel(props.prompt.id)}>
              Cancel
            </button>
            <button
              class="button-primary"
              disabled={!response().trim()}
              onClick={() => props.onSubmit(props.prompt.id, response())}
            >
              Submit
            </button>
          </div>
        </Show>
      </div>
    </div>
  );
};
