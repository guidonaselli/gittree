import { type Component, Show, createSignal } from "solid-js";
import type { CommitOptions } from "../../api/commands";

export const CommitPanel: Component<{
  stagedCount: number;
  onCommit: (message: string, options: CommitOptions) => Promise<void>;
  error: string | null;
}> = (props) => {
  const [message, setMessage] = createSignal("");
  const [author, setAuthor] = createSignal("");
  const [signOff, setSignOff] = createSignal(false);
  const [sign, setSign] = createSignal(false);
  const [committing, setCommitting] = createSignal(false);

  async function submit() {
    if (!message().trim() || props.stagedCount === 0 || committing()) return;
    setCommitting(true);
    try {
      await props.onCommit(message(), {
        author: author().trim() || undefined,
        signOff: signOff(),
        sign: sign(),
      });
      setMessage("");
    } finally {
      setCommitting(false);
    }
  }

  return (
    <section class="working-copy-group commit-panel">
      <h3>Commit</h3>
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
      <button
        class="working-copy-action"
        disabled={!message().trim() || props.stagedCount === 0 || committing()}
        onClick={() => void submit()}
      >
        {committing() ? "Committing…" : `Commit ${props.stagedCount} staged file${props.stagedCount === 1 ? "" : "s"}`}
      </button>
    </section>
  );
};
