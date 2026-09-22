import { createSignal, createResource, Show, For, type Component } from "solid-js";
import { getRemotes, pushRepository, cancelSyncNetworkOperation } from "../../api/commands";
import type { PushForceMode, PushOptions, RemoteInfo, PushOutcome } from "../../api/types";
import { PushRejectedModal } from "./PushRejectedModal";
import { useModalEscape } from "../../layout/modal-escape";
import "./sync.css";

export interface PushModalProps {
  root: string;
  currentBranch?: string | null;
  onClose: () => void;
  onSuccess: () => void;
  onOpenPull: () => void;
}

export const PushModal: Component<PushModalProps> = (props) => {
  useModalEscape(props.onClose);
  const [remotes] = createResource(() => props.root, getRemotes);
  const [selectedRemote, setSelectedRemote] = createSignal<string>("origin");
  const [refspec, setRefspec] = createSignal<string>(props.currentBranch ?? "");
  const [setUpstream, setSetUpstream] = createSignal<boolean>(true);
  const [tags, setTags] = createSignal<boolean>(false);

  // Force mode: none | force_with_lease | bare_force
  const [forceKind, setForceKind] = createSignal<"none" | "force_with_lease" | "bare_force">("none");
  const [bareForceAcknowledged, setBareForceAcknowledged] = createSignal<boolean>(false);

  const [inProgress, setInProgress] = createSignal<boolean>(false);
  const [pushOutcome, setPushOutcome] = createSignal<PushOutcome | null>(null);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [rejectedMessage, setRejectedMessage] = createSignal<string | null>(null);

  const canSubmit = () => {
    if (inProgress()) return false;
    if (forceKind() === "bare_force" && !bareForceAcknowledged()) {
      return false;
    }
    return true;
  };

  const handlePush = async () => {
    if (!canSubmit()) return;

    setInProgress(true);
    setErrorMessage(null);
    setPushOutcome(null);
    setRejectedMessage(null);

    let force_mode: PushForceMode = { mode: "none" };
    if (forceKind() === "force_with_lease") {
      force_mode = { mode: "force_with_lease" };
    } else if (forceKind() === "bare_force") {
      force_mode = {
        mode: "bare_force",
        acknowledged_destructive: bareForceAcknowledged(),
      };
    }

    const options: PushOptions = {
      remote: selectedRemote(),
      refspec: refspec().trim() ? refspec().trim() : null,
      force_mode,
      tags: tags(),
      set_upstream: setUpstream(),
    };

    try {
      const outcome = await pushRepository(props.root, options);
      setPushOutcome(outcome);

      if (outcome.status === "success") {
        props.onSuccess();
      } else if (outcome.status === "rejected_non_fast_forward") {
        setRejectedMessage(outcome.remote_message);
      }
    } catch (err) {
      setErrorMessage(String(err));
    } finally {
      setInProgress(false);
    }
  };

  const handleCancelOperation = async () => {
    try {
      await cancelSyncNetworkOperation();
    } catch {
      // ignore
    }
  };

  return (
    <>
      <Show when={rejectedMessage()}>
        {(msg) => (
          <PushRejectedModal
            remoteMessage={msg()}
            onPullThenRetry={() => {
              props.onClose();
              props.onOpenPull();
            }}
            onClose={() => setRejectedMessage(null)}
          />
        )}
      </Show>

      <Show when={!rejectedMessage()}>
        <div class="sync-modal-overlay" role="dialog" aria-modal="true" aria-labelledby="push-modal-title">
          <div class="sync-modal-content">
            <div class="sync-modal-header">
              <h2 id="push-modal-title" class="sync-modal-title">
                Push to Remote
              </h2>
              <button class="collapse-toggle" onClick={props.onClose} aria-label="Close">
                ✕
              </button>
            </div>

            <Show when={!inProgress() && !pushOutcome()}>
              <div class="sync-form-group">
                <label class="sync-form-label" for="push-remote-select">
                  Remote
                </label>
                <select
                  id="push-remote-select"
                  class="sync-form-select"
                  value={selectedRemote()}
                  onChange={(e) => setSelectedRemote(e.currentTarget.value)}
                >
                  <For each={remotes() ?? []}>
                    {(r: RemoteInfo) => <option value={r.name}>{r.name} ({r.push_url || r.fetch_url || "no URL"})</option>}
                  </For>
                </select>
              </div>

              <div class="sync-form-group">
                <label class="sync-form-label" for="push-refspec-input">
                  Branch / Refspec
                </label>
                <input
                  id="push-refspec-input"
                  class="sync-form-input"
                  type="text"
                  placeholder={props.currentBranch || "e.g. main"}
                  value={refspec()}
                  onInput={(e) => setRefspec(e.currentTarget.value)}
                />
              </div>

              <div class="sync-form-group">
                <label class="sync-checkbox-label">
                  <input
                    type="checkbox"
                    checked={setUpstream()}
                    onChange={(e) => setSetUpstream(e.currentTarget.checked)}
                  />
                  Set upstream tracking (-u / --set-upstream)
                </label>
                <label class="sync-checkbox-label">
                  <input
                    type="checkbox"
                    checked={tags()}
                    onChange={(e) => setTags(e.currentTarget.checked)}
                  />
                  Push all tags (--tags)
                </label>
              </div>

              <div class="sync-form-group">
                <span class="sync-form-label">Push Safety & Overwrite Options</span>
                <div class="sync-radio-group">
                  <label class="sync-radio-label">
                    <input
                      type="radio"
                      name="force-kind"
                      value="none"
                      checked={forceKind() === "none"}
                      onChange={() => setForceKind("none")}
                    />
                    <div class="sync-radio-text">
                      <span class="sync-radio-title">Standard Safe Push</span>
                      <span class="sync-radio-desc">Refuses push if remote contains unseen commits.</span>
                    </div>
                  </label>

                  <label class="sync-radio-label">
                    <input
                      type="radio"
                      name="force-kind"
                      value="force_with_lease"
                      checked={forceKind() === "force_with_lease"}
                      onChange={() => setForceKind("force_with_lease")}
                    />
                    <div class="sync-radio-text">
                      <span class="sync-radio-title">Force with lease (--force-with-lease)</span>
                      <span class="sync-radio-desc">
                        Default force path. Overwrites remote branch only if nobody else pushed to it in the meantime.
                      </span>
                    </div>
                  </label>

                  <label class="sync-radio-label">
                    <input
                      type="radio"
                      name="force-kind"
                      value="bare_force"
                      checked={forceKind() === "bare_force"}
                      onChange={() => setForceKind("bare_force")}
                    />
                    <div class="sync-radio-text">
                      <span class="sync-radio-title text-danger">Bare Force Push (--force)</span>
                      <span class="sync-radio-desc text-danger">
                        Destructive. Overwrites remote history unconditionally.
                      </span>
                    </div>
                  </label>
                </div>
              </div>

              <Show when={forceKind() === "bare_force"}>
                <div class="sync-danger-alert">
                  <div class="sync-danger-title">⚠️ DESTRUCTIVE ACTION WARNING</div>
                  <div>
                    Bare force push can destroy commits made by other contributors on the remote repository.
                    GitTree requires explicit acknowledgement before proceeding.
                  </div>
                  <label class="sync-checkbox-label" style={{ "margin-top": "var(--space-2)" }}>
                    <input
                      type="checkbox"
                      checked={bareForceAcknowledged()}
                      onChange={(e) => setBareForceAcknowledged(e.currentTarget.checked)}
                    />
                    <strong>I understand and acknowledge this can destroy others' commits on the remote.</strong>
                  </label>
                </div>
              </Show>

              <Show when={errorMessage()}>
                {(err) => <div class="sync-danger-alert">{err()}</div>}
              </Show>

              <div class="sync-modal-actions">
                <button class="collapse-toggle" onClick={props.onClose}>
                  Cancel
                </button>
                <button
                  class="button-primary"
                  disabled={!canSubmit()}
                  onClick={handlePush}
                >
                  Push
                </button>
              </div>
            </Show>

            <Show when={inProgress()}>
              <div class="sync-progress-box">
                <span>Pushing objects and updating remote refs...</span>
                <button class="collapse-toggle text-danger" onClick={handleCancelOperation}>
                  Cancel / Abort Push
                </button>
              </div>
            </Show>

            <Show when={pushOutcome()}>
              {(outcome) => (
                <div class="sync-form-group">
                  <Show when={outcome().status === "success"}>
                    <div class="sync-outcome-item success">
                      <div class="sync-outcome-header">
                        <span>Push Successful</span>
                        <span>✓</span>
                      </div>
                      <div class="sync-outcome-summary">
                        {(outcome() as { summary: string }).summary}
                      </div>
                      <Show when={(outcome() as { details: string }).details}>
                        <div class="sync-verbatim-error">
                          {(outcome() as { details: string }).details}
                        </div>
                      </Show>
                    </div>
                  </Show>

                  <Show when={outcome().status === "cancelled"}>
                    <div class="sync-danger-alert">
                      <div class="sync-danger-title">Push Cancelled</div>
                      <div>{(outcome() as { summary: string }).summary}</div>
                    </div>
                  </Show>

                  <Show when={outcome().status === "failed"}>
                    <div class="sync-danger-alert">
                      <div class="sync-danger-title">Push Failed</div>
                      <div class="sync-verbatim-error">{(outcome() as { message: string }).message}</div>
                    </div>
                  </Show>

                  <div class="sync-modal-actions">
                    <button class="button-primary" onClick={props.onClose}>
                      Close
                    </button>
                  </div>
                </div>
              )}
            </Show>
          </div>
        </div>
      </Show>
    </>
  );
};
