import {
  type Component,
  createEffect,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import {
  type KeybindingAction,
  KEYBINDING_ACTIONS,
  normalizeShortcut,
  validateRebind,
} from "./keybindings";
import "./keybindings.css";

export interface KeybindingsModalProps {
  open: boolean;
  onClose: () => void;
  customBindings: Record<string, string>;
  onSaveBindings: (bindings: Record<string, string>) => void;
}

export const KeybindingsModal: Component<KeybindingsModalProps> = (props) => {
  const [listeningActionId, setListeningActionId] = createSignal<string | null>(null);
  const [conflictMessage, setConflictMessage] = createSignal<string | null>(null);
  let previousActiveElement: HTMLElement | null = null;

  createEffect(() => {
    if (props.open) {
      previousActiveElement = document.activeElement as HTMLElement | null;
      setListeningActionId(null);
      setConflictMessage(null);
    } else {
      setListeningActionId(null);
      if (previousActiveElement && typeof previousActiveElement.focus === "function") {
        previousActiveElement.focus();
      }
    }
  });

  function startListening(actionId: string) {
    setListeningActionId(actionId);
    setConflictMessage(null);
  }

  function handleRebind(actionId: string, event: KeyboardEvent) {
    // Ignore pure modifier presses
    if (["Control", "Alt", "Shift", "Meta"].includes(event.key)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      setListeningActionId(null);
      return;
    }

    const modifiers: string[] = [];
    if (event.ctrlKey) modifiers.push("Ctrl");
    if (event.altKey) modifiers.push("Alt");
    if (event.shiftKey) modifiers.push("Shift");
    if (event.metaKey) modifiers.push("Meta");

    const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
    const raw = [...modifiers, key].join("+");
    const candidate = normalizeShortcut(raw);

    const outcome = validateRebind(actionId, candidate, props.customBindings);
    if (!outcome.success) {
      const conflictName = outcome.conflictingAction?.title ?? "another action";
      setConflictMessage(
        `Shortcut '${candidate}' conflicts with '${conflictName}'. Choose a different shortcut.`
      );
      setListeningActionId(null);
      return;
    }

    // Success: save binding
    const updated = { ...props.customBindings, [actionId]: candidate };
    props.onSaveBindings(updated);
    setListeningActionId(null);
    setConflictMessage(null);
  }

  function resetAction(actionId: string) {
    const updated = { ...props.customBindings };
    delete updated[actionId];
    props.onSaveBindings(updated);
    setConflictMessage(null);
  }

  function resetAll() {
    props.onSaveBindings({});
    setConflictMessage(null);
  }

  onMount(() => {
    const onGlobalKeyDown = (e: KeyboardEvent) => {
      if (!props.open) return;

      const listeningId = listeningActionId();
      if (listeningId) {
        handleRebind(listeningId, e);
        return;
      }

      if (e.key === "Escape") {
        e.preventDefault();
        props.onClose();
      }
    };

    window.addEventListener("keydown", onGlobalKeyDown, true);
    onCleanup(() => window.removeEventListener("keydown", onGlobalKeyDown, true));
  });

  const categories = () => {
    const cats: string[] = [];
    for (const a of KEYBINDING_ACTIONS) {
      if (!cats.includes(a.category)) cats.push(a.category);
    }
    return cats;
  };

  const actionsForCategory = (cat: string) => {
    return KEYBINDING_ACTIONS.filter((a) => a.category === cat);
  };

  return (
    <Show when={props.open}>
      <div
        class="keybindings-backdrop"
        onClick={(e) => {
          if (e.target === e.currentTarget) props.onClose();
        }}
        role="presentation"
      >
        <div
          class="keybindings-modal"
          role="dialog"
          aria-modal="true"
          aria-label="Keyboard Shortcuts"
        >
          <div class="keybindings-header">
            <h2>Keyboard Shortcuts</h2>
            <button
              type="button"
              class="keybindings-close-icon-btn"
              onClick={props.onClose}
              title="Close (Esc)"
              aria-label="Close"
            >
              ✕
            </button>
          </div>

          <Show when={conflictMessage()}>
            {(msg) => (
              <div class="keybindings-conflict-banner" role="alert">
                <span>⚠ {msg()}</span>
                <button type="button" onClick={() => setConflictMessage(null)}>
                  ✕
                </button>
              </div>
            )}
          </Show>

          <div class="keybindings-body">
            <For each={categories()}>
              {(category) => (
                <div>
                  <div class="keybindings-group-title">{category}</div>
                  <For each={actionsForCategory(category)}>
                    {(action: KeybindingAction) => {
                      const isListening = () => listeningActionId() === action.id;
                      const custom = () => props.customBindings[action.id];
                      const effective = () => custom() ?? action.defaultShortcut;

                      return (
                        <div class="keybindings-row">
                          <div class="keybindings-action-info">
                            <span class="keybindings-action-title">{action.title}</span>
                            <Show when={action.description}>
                              <span class="keybindings-action-desc">{action.description}</span>
                            </Show>
                          </div>

                          <div class="keybindings-controls">
                            <Show when={custom()}>
                              <span class="keybindings-custom-badge">Custom</span>
                            </Show>

                            <Show
                              when={isListening()}
                              fallback={<kbd class="keybindings-kbd">{effective()}</kbd>}
                            >
                              <span class="keybindings-listening-badge">
                                Press keys… (Esc to cancel)
                              </span>
                            </Show>

                            <button
                              type="button"
                              class="keybindings-btn"
                              classList={{ "btn-active": isListening() }}
                              onClick={() => {
                                if (isListening()) setListeningActionId(null);
                                else startListening(action.id);
                              }}
                            >
                              {isListening() ? "Cancel" : "Rebind"}
                            </button>

                            <Show when={custom()}>
                              <button
                                type="button"
                                class="keybindings-reset-btn"
                                onClick={() => resetAction(action.id)}
                                title="Reset to default"
                              >
                                Reset
                              </button>
                            </Show>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              )}
            </For>
          </div>

          <div class="keybindings-footer">
            <button
              type="button"
              class="keybindings-reset-all-btn"
              onClick={resetAll}
              title="Reset all shortcuts to factory defaults"
            >
              Reset All to Defaults
            </button>
            <button
              type="button"
              class="keybindings-done-btn"
              onClick={props.onClose}
            >
              Done
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
};
