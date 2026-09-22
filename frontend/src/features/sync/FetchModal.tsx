import { createSignal, createResource, Show, For, type Component } from "solid-js";
import { getRemotes, fetchRemotes, cancelSyncNetworkOperation } from "../../api/commands";
import type { MultiRemoteFetchResult, RemoteInfo } from "../../api/types";
import { useModalEscape } from "../../layout/modal-escape";
import "./sync.css";

export interface FetchModalProps {
  root: string;
  onClose: () => void;
  onSuccess: () => void;
}

export const FetchModal: Component<FetchModalProps> = (props) => {
  useModalEscape(props.onClose);
  const [remotes] = createResource(() => props.root, getRemotes);
  const [selectedRemote, setSelectedRemote] = createSignal<string>("all");
  const [prune, setPrune] = createSignal<boolean>(true);
  const [tags, setTags] = createSignal<boolean>(false);
  const [refspec, setRefspec] = createSignal<string>("");

  const [inProgress, setInProgress] = createSignal<boolean>(false);
  const [fetchResult, setFetchResult] = createSignal<MultiRemoteFetchResult | null>(null);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);

  const handleFetch = async () => {
    setInProgress(true);
    setErrorMessage(null);
    setFetchResult(null);

    try {
      const remote = selectedRemote() === "all" ? null : selectedRemote();
      const result = await fetchRemotes(props.root, {
        remote,
        prune: prune(),
        tags: tags(),
        refspec: refspec().trim() ? refspec().trim() : null,
      });
      setFetchResult(result);
      if (result.succeeded > 0) {
        props.onSuccess();
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
    <div class="sync-modal-overlay" role="dialog" aria-modal="true" aria-labelledby="fetch-modal-title">
      <div class="sync-modal-content">
        <div class="sync-modal-header">
          <h2 id="fetch-modal-title" class="sync-modal-title">
            Fetch Remotes
          </h2>
          <button class="collapse-toggle" onClick={props.onClose} aria-label="Close">
            ✕
          </button>
        </div>

        <Show when={!inProgress() && !fetchResult()}>
          <div class="sync-form-group">
            <label class="sync-form-label" for="fetch-remote-select">
              Remote
            </label>
            <select
              id="fetch-remote-select"
              class="sync-form-select"
              value={selectedRemote()}
              onChange={(e) => setSelectedRemote(e.currentTarget.value)}
            >
              <option value="all">All configured remotes (concurrent)</option>
              <For each={remotes() ?? []}>
                {(r: RemoteInfo) => <option value={r.name}>{r.name} ({r.fetch_url || "no URL"})</option>}
              </For>
            </select>
          </div>

          <div class="sync-form-group">
            <label class="sync-form-label" for="fetch-refspec-input">
              Refspec (Optional)
            </label>
            <input
              id="fetch-refspec-input"
              class="sync-form-input"
              type="text"
              placeholder="e.g. refs/heads/*:refs/remotes/origin/*"
              value={refspec()}
              onInput={(e) => setRefspec(e.currentTarget.value)}
            />
          </div>

          <div class="sync-form-group">
            <label class="sync-checkbox-label">
              <input
                type="checkbox"
                checked={prune()}
                onChange={(e) => setPrune(e.currentTarget.checked)}
              />
              Prune remote-tracking branches no longer on remote (--prune)
            </label>
            <label class="sync-checkbox-label">
              <input
                type="checkbox"
                checked={tags()}
                onChange={(e) => setTags(e.currentTarget.checked)}
              />
              Fetch all remote tags (--tags)
            </label>
          </div>

          <Show when={errorMessage()}>
            {(err) => <div class="sync-danger-alert">{err()}</div>}
          </Show>

          <div class="sync-modal-actions">
            <button class="collapse-toggle" onClick={props.onClose}>
              Cancel
            </button>
            <button class="button-primary" onClick={handleFetch}>
              Fetch
            </button>
          </div>
        </Show>

        <Show when={inProgress()}>
          <div class="sync-progress-box">
            <span>Fetching objects and refs from remote...</span>
            <button class="collapse-toggle text-danger" onClick={handleCancelOperation}>
              Cancel / Abort Fetch
            </button>
          </div>
        </Show>

        <Show when={fetchResult()}>
          {(res) => (
            <div class="sync-form-group">
              <div class="sync-radio-desc">
                Fetched {res().total} remote(s): {res().succeeded} succeeded, {res().failed} failed.
              </div>
              <div class="sync-outcome-list">
                <For each={res().results}>
                  {(outcome) => (
                    <div class={`sync-outcome-item ${outcome.success ? "success" : "failed"}`}>
                      <div class="sync-outcome-header">
                        <span>{outcome.remote}</span>
                        <span>{outcome.success ? "✓ Succeeded" : "✗ Failed"}</span>
                      </div>
                      <div class="sync-outcome-summary">{outcome.summary}</div>
                      <Show when={outcome.error}>
                        <div class="sync-outcome-error">{outcome.error}</div>
                      </Show>
                    </div>
                  )}
                </For>
              </div>
              <div class="sync-modal-actions">
                <button class="button-primary" onClick={props.onClose}>
                  Done
                </button>
              </div>
            </div>
          )}
        </Show>
      </div>
    </div>
  );
};
