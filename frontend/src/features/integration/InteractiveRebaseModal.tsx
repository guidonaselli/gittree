import {
  type Component,
  For,
  Show,
  createEffect,
  createResource,
  createSignal,
} from "solid-js";
import { createStash, getRebasePlan, startInteractiveRebase } from "../../api/commands";
import type {
  DirtyTreeDetails,
  RebaseAction,
  RebasePlanItem,
} from "../../api/types";
import { DirtyTreeRefusalModal } from "./DirtyTreeRefusalModal";
import "./integration.css";

export interface InteractiveRebaseModalProps {
  root: string;
  baseRef: string;
  baseDescription?: string;
  onClose: () => void;
  onSuccess: () => void;
  onPaused: (conflicts: string[], message: string) => void;
  onNavigateWorkingCopy?: () => void;
}

export const InteractiveRebaseModal: Component<InteractiveRebaseModalProps> = (props) => {
  const [initialPlan] = createResource(
    () => ({ root: props.root, base: props.baseRef }),
    async ({ root, base }) => {
      return await getRebasePlan(root, base);
    }
  );

  const [items, setItems] = createSignal<RebasePlanItem[]>([]);
  const [dirtyDetails, setDirtyDetails] = createSignal<DirtyTreeDetails | null>(null);
  const [submitting, setSubmitting] = createSignal(false);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);

  createEffect(() => {
    const list = initialPlan();
    if (list) {
      setItems(list.map((item) => ({ ...item })));
    }
  });

  function updateAction(index: number, action: RebaseAction) {
    setItems((prev) => {
      const next = [...prev];
      next[index] = { ...next[index], action };
      return next;
    });
  }

  function updateMessage(index: number, msg: string) {
    setItems((prev) => {
      const next = [...prev];
      next[index] = { ...next[index], new_message: msg };
      return next;
    });
  }

  function moveItem(index: number, direction: "up" | "down") {
    setItems((prev) => {
      const next = [...prev];
      const target = direction === "up" ? index - 1 : index + 1;
      if (target < 0 || target >= next.length) return prev;
      const temp = next[index];
      next[index] = next[target];
      next[target] = temp;
      return next;
    });
  }

  async function executeRebase() {
    setSubmitting(true);
    setErrorMessage(null);
    setDirtyDetails(null);

    try {
      const outcome = await startInteractiveRebase(props.root, props.baseRef, items());
      if (outcome.status === "Success") {
        props.onSuccess();
      } else if (outcome.status === "Paused") {
        props.onPaused(outcome.conflicting_files, outcome.message);
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
        message: `Auto-stash before rebase onto ${props.baseRef}`,
        include_untracked: true,
        keep_index: false,
      });
      await executeRebase();
    } catch (e) {
      setErrorMessage(String(e));
    }
  }

  const activeCount = () => items().filter((i) => i.action !== "Drop").length;
  const dropCount = () => items().filter((i) => i.action === "Drop").length;

  return (
    <>
      <div class="integration-modal-overlay">
        <div class="integration-modal-card">
          <div class="integration-modal-header">
            <h3>Interactive Rebase onto {props.baseDescription || props.baseRef}</h3>
            <button class="integration-btn" onClick={props.onClose}>
              ✕
            </button>
          </div>

          <div class="integration-modal-body">
            <Show when={errorMessage()}>
              <div class="integration-error-bar">{errorMessage()}</div>
            </Show>

            <div class="integration-banner-desc">
              Reorder, pick, reword, edit, squash, fixup, or drop commits from the range. Confirm
              the plan below before execution.
            </div>

            <Show when={initialPlan.loading}>
              <p class="integration-banner-desc">Loading commit range...</p>
            </Show>

            <Show when={!initialPlan.loading && items().length === 0}>
              <div class="integration-notice-bar">
                No commits found between {props.baseRef} and HEAD. Repository is already up to date.
              </div>
            </Show>

            <Show when={items().length > 0}>
              <div class="rebase-plan-table-container">
                <For each={items()}>
                  {(item, idx) => {
                    const isDrop = () => item.action === "Drop";
                    const isReword = () => item.action === "Reword";
                    const isSquash = () => item.action === "Squash";

                    return (
                      <div class={`rebase-plan-item ${isDrop() ? "rebase-plan-item-drop" : ""}`}>
                        <div class="rebase-reorder-group">
                          <button
                            class="rebase-icon-btn"
                            disabled={idx() === 0 || submitting()}
                            onClick={() => moveItem(idx(), "up")}
                            title="Move up"
                          >
                            ▲
                          </button>
                          <button
                            class="rebase-icon-btn"
                            disabled={idx() === items().length - 1 || submitting()}
                            onClick={() => moveItem(idx(), "down")}
                            title="Move down"
                          >
                            ▼
                          </button>
                        </div>

                        <select
                          class="rebase-action-select"
                          value={item.action}
                          disabled={submitting()}
                          onChange={(e) =>
                            updateAction(idx(), e.currentTarget.value as RebaseAction)
                          }
                        >
                          <option value="Pick">Pick</option>
                          <option value="Reword">Reword</option>
                          <option value="Edit">Edit</option>
                          <option value="Squash">Squash</option>
                          <option value="Fixup">Fixup</option>
                          <option value="Drop">Drop</option>
                        </select>

                        <span class="rebase-commit-sha">{item.short_commit}</span>

                        <div style={{ flex: 1, "min-width": 0 }}>
                          <div class="rebase-commit-subject" title={item.subject}>
                            {item.subject}
                          </div>
                          <Show when={isReword() || isSquash()}>
                            <input
                              type="text"
                              class="rebase-reword-input"
                              placeholder={
                                isReword()
                                  ? "New commit message..."
                                  : "Combined squash commit message..."
                              }
                              value={item.new_message ?? ""}
                              onInput={(e) => updateMessage(idx(), e.currentTarget.value)}
                            />
                          </Show>
                        </div>

                        <span class="rebase-commit-author">{item.author}</span>
                      </div>
                    );
                  }}
                </For>
              </div>

              <div class="integration-banner-desc">
                Plan summary: <strong>{activeCount()}</strong> commit(s) to apply,{" "}
                <strong>{dropCount()}</strong> commit(s) dropped.
              </div>
            </Show>
          </div>

          <div class="integration-modal-footer">
            <button
              class="integration-btn integration-btn-primary"
              disabled={submitting() || items().length === 0}
              onClick={() => void executeRebase()}
            >
              {submitting() ? "Rebasing..." : "Confirm & Start Rebase"}
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
            operationName="Interactive Rebase"
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
