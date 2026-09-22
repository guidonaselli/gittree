import { createSignal, createResource, Show, For, type Component } from "solid-js";
import { getRemotes, pullRepository, cancelSyncNetworkOperation } from "../../api/commands";
import type { PullStrategy, RemoteInfo, PullOutcome } from "../../api/types";
import { useModalEscape } from "../../layout/modal-escape";
import "./sync.css";

export interface PullModalProps {
  root: string;
  currentBranch?: string | null;
  onClose: () => void;
  onSuccess: () => void;
  onConflict: (files: string[]) => void;
}

export const PullModal: Component<PullModalProps> = (props) => {
  useModalEscape(props.onClose);
  const [remotes] = createResource(() => props.root, getRemotes);
  const [selectedRemote, setSelectedRemote] = createSignal<string>("origin");
  const [branch, setBranch] = createSignal<string>(props.currentBranch ?? "");
  const [strategy, setStrategy] = createSignal<PullStrategy>("rebase");
  const [prune, setPrune] = createSignal<boolean>(true);
  const [tags, setTags] = createSignal<boolean>(false);

  const [inProgress, setInProgress] = createSignal<boolean>(false);
  const [pullOutcome, setPullOutcome] = createSignal<PullOutcome | null>(null);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);

  const handlePull = async () => {
    setInProgress(true);
    setErrorMessage(null);
    setPullOutcome(null);

    try {
      const outcome = await pullRepository(props.root, {
        remote: selectedRemote(),
        branch: branch().trim() ? branch().trim() : null,
        strategy: strategy(),
        prune: prune(),
        tags: tags(),
      });

      setPullOutcome(outcome);

      if (outcome.status === "success") {
        props.onSuccess();
      } else if (outcome.status === "conflict") {
        props.onConflict(outcome.conflicting_files);
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
    <div class="sync-modal-overlay" role="dialog" aria-modal="true" aria-labelledby="pull-modal-title">
      <div class="sync-modal-content">
        <div class="sync-modal-header">
          <h2 id="pull-modal-title" class="sync-modal-title">
            Pull from Remote
          </h2>
          <button class="collapse-toggle" onClick={props.onClose} aria-label="Close">
            ✕
          </button>
        </div>

        <Show when={!inProgress() && !pullOutcome()}>
          <div class="sync-form-group">
            <label class="sync-form-label" for="pull-remote-select">
              Remote
            </label>
            <select
              id="pull-remote-select"
              class="sync-form-select"
              value={selectedRemote()}
              onChange={(e) => setSelectedRemote(e.currentTarget.value)}
            >
              <For each={remotes() ?? []}>
                {(r: RemoteInfo) => <option value={r.name}>{r.name} ({r.fetch_url || "no URL"})</option>}
              </For>
            </select>
          </div>

          <div class="sync-form-group">
            <label class="sync-form-label" for="pull-branch-input">
              Remote Branch (Optional)
            </label>
            <input
              id="pull-branch-input"
              class="sync-form-input"
              type="text"
              placeholder={props.currentBranch || "e.g. main"}
              value={branch()}
              onInput={(e) => setBranch(e.currentTarget.value)}
            />
          </div>

          <div class="sync-form-group">
            <span class="sync-form-label">Integration Strategy</span>
            <div class="sync-radio-group">
              <label class="sync-radio-label">
                <input
                  type="radio"
                  name="pull-strategy"
                  value="rebase"
                  checked={strategy() === "rebase"}
                  onChange={() => setStrategy("rebase")}
                />
                <div class="sync-radio-text">
                  <span class="sync-radio-title">Rebase local commits onto remote (--rebase)</span>
                  <span class="sync-radio-desc">Preserves linear history without extra merge commits.</span>
                </div>
              </label>

              <label class="sync-radio-label">
                <input
                  type="radio"
                  name="pull-strategy"
                  value="merge"
                  checked={strategy() === "merge"}
                  onChange={() => setStrategy("merge")}
                />
                <div class="sync-radio-text">
                  <span class="sync-radio-title">Merge remote into local (--no-rebase)</span>
                  <span class="sync-radio-desc">Creates a merge commit joining histories.</span>
                </div>
              </label>

              <label class="sync-radio-label">
                <input
                  type="radio"
                  name="pull-strategy"
                  value="fast_forward_only"
                  checked={strategy() === "fast_forward_only"}
                  onChange={() => setStrategy("fast_forward_only")}
                />
                <div class="sync-radio-text">
                  <span class="sync-radio-title">Fast-Forward Only (--ff-only)</span>
                  <span class="sync-radio-desc">Refuses pull if histories have diverged.</span>
                </div>
              </label>
            </div>
          </div>

          <div class="sync-form-group">
            <label class="sync-checkbox-label">
              <input
                type="checkbox"
                checked={prune()}
                onChange={(e) => setPrune(e.currentTarget.checked)}
              />
              Prune stale tracking branches (--prune)
            </label>
            <label class="sync-checkbox-label">
              <input
                type="checkbox"
                checked={tags()}
                onChange={(e) => setTags(e.currentTarget.checked)}
              />
              Fetch remote tags (--tags)
            </label>
          </div>

          <Show when={errorMessage()}>
            {(err) => <div class="sync-danger-alert">{err()}</div>}
          </Show>

          <div class="sync-modal-actions">
            <button class="collapse-toggle" onClick={props.onClose}>
              Cancel
            </button>
            <button class="button-primary" onClick={handlePull}>
              Pull
            </button>
          </div>
        </Show>

        <Show when={inProgress()}>
          <div class="sync-progress-box">
            <span>Pulling from remote repository...</span>
            <button class="collapse-toggle text-danger" onClick={handleCancelOperation}>
              Cancel / Abort Pull
            </button>
          </div>
        </Show>

        <Show when={pullOutcome()}>
          {(outcome) => (
            <div class="sync-form-group">
              <Show when={outcome().status === "success"}>
                <div class="sync-outcome-item success">
                  <div class="sync-outcome-header">
                    <span>Pull Successful</span>
                    <span>✓</span>
                  </div>
                  <div class="sync-outcome-summary">
                    {(outcome() as { summary: string }).summary}
                  </div>
                </div>
              </Show>

              <Show when={outcome().status === "conflict"}>
                <div class="sync-danger-alert">
                  <div class="sync-danger-title">Merge Conflicts Encountered</div>
                  <div>{(outcome() as { summary: string }).summary}</div>
                  <div class="sync-outcome-summary">
                    Conflicted files: {(outcome() as { conflicting_files: string[] }).conflicting_files.join(", ")}
                  </div>
                </div>
              </Show>

              <Show when={outcome().status === "cancelled"}>
                <div class="sync-danger-alert">
                  <div class="sync-danger-title">Operation Cancelled</div>
                  <div>{(outcome() as { summary: string }).summary}</div>
                </div>
              </Show>

              <Show when={outcome().status === "failed"}>
                <div class="sync-danger-alert">
                  <div class="sync-danger-title">Pull Failed</div>
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
  );
};
