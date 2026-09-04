import { type Component, Show, createSignal } from "solid-js";
import type { CommitOptions } from "../../api/commands";

export const CommitPanel: Component<{
  stagedCount: number;
  onCommit: (message: string, options: CommitOptions) => Promise<void>;
  onCheckHeadPublished: () => Promise<boolean>;
  onAmend: (message: string, options: CommitOptions) => Promise<void>;
  error: string | null;
  hookOutput: string | null;
}> = (props) => {
  const [message, setMessage] = createSignal("");
  const [author, setAuthor] = createSignal("");
  const [signOff, setSignOff] = createSignal(false);
  const [sign, setSign] = createSignal(false);
  const [amendMode, setAmendMode] = createSignal(false);
  const [committing, setCommitting] = createSignal(false);
  const [publishedWarning, setPublishedWarning] = createSignal(false);

  function options(): CommitOptions {
    return { author: author().trim() || undefined, signOff: signOff(), sign: sign() };
  }

  async function doAmend() {
    setCommitting(true);
    try {
      await props.onAmend(message(), options());
      setMessage("");
      setPublishedWarning(false);
    } finally {
      setCommitting(false);
    }
  }

  async function submit() {
    if (!message().trim() || committing()) return;
    if (!amendMode() && props.stagedCount === 0) return;

    if (amendMode()) {
      setCommitting(true);
      const published = await props.onCheckHeadPublished().finally(() => setCommitting(false));
      if (published) {
        setPublishedWarning(true);
        return;
      }
      await doAmend();
      return;
    }

    setCommitting(true);
    try {
      await props.onCommit(message(), options());
      setMessage("");
    } finally {
      setCommitting(false);
    }
  }

  return (
    <section class="working-copy-group commit-panel">
      <h3>Commit</h3>
      <label class="commit-checkbox">
        <input
          type="checkbox"
          checked={amendMode()}
          onChange={(e) => {
            setAmendMode(e.currentTarget.checked);
            setPublishedWarning(false);
          }}
        />
        Amend previous commit
      </label>
      <textarea
        class="commit-message-input"
        placeholder="Commit message"
        rows={3}
        value={message()}
        onInput={(e) => setMessage(e.currentTarget.value)}
      />
      <input
        class="commit-author-input"
        type="text"
        placeholder="Author override (Name <email>), optional"
        value={author()}
        onInput={(e) => setAuthor(e.currentTarget.value)}
      />
      <label class="commit-checkbox">
        <input type="checkbox" checked={signOff()} onChange={(e) => setSignOff(e.currentTarget.checked)} />
        Sign off
      </label>
      <label class="commit-checkbox">
        <input type="checkbox" checked={sign()} onChange={(e) => setSign(e.currentTarget.checked)} />
        GPG/SSH sign
      </label>
      <Show when={props.error}>
        <p class="diff-apply-error">{props.error}</p>
      </Show>
      <Show when={props.hookOutput}>
        <pre class="commit-hook-output">{props.hookOutput}</pre>
      </Show>
      <Show when={publishedWarning()}>
        <div class="discard-confirm">
          <p>
            This commit has already been pushed to its upstream. Amending it rewrites published history — anyone who
            already fetched it will need a force-push to catch up.
          </p>
          <div class="discard-confirm-actions">
            <button class="working-copy-action working-copy-action-danger" onClick={() => void doAmend()}>
              Amend anyway
            </button>
            <button class="working-copy-action" onClick={() => setPublishedWarning(false)}>
              Cancel
            </button>
          </div>
        </div>
      </Show>
      <button
        class="working-copy-action"
        disabled={!message().trim() || (!amendMode() && props.stagedCount === 0) || committing()}
        onClick={() => void submit()}
      >
        {committing()
          ? "Working…"
          : amendMode()
            ? "Amend commit"
            : `Commit ${props.stagedCount} staged file${props.stagedCount === 1 ? "" : "s"}`}
      </button>
    </section>
  );
};
