import { type Component, Show, createEffect, createSignal } from "solid-js";
import type { CommitMessageTemplate, CommitOptions } from "../../api/commands";

interface Draft {
  message: string;
  author: string;
  signOff: boolean;
  sign: boolean;
}

function draftKey(root: string): string {
  return `gittree.commit-draft.${root}`;
}

function loadDraft(root: string): Draft | null {
  try {
    const raw = localStorage.getItem(draftKey(root));
    return raw !== null ? (JSON.parse(raw) as Draft) : null;
  } catch {
    return null;
  }
}

function saveDraft(root: string, draft: Draft) {
  try {
    localStorage.setItem(draftKey(root), JSON.stringify(draft));
  } catch {
    // Draft persistence is a convenience; storage failures are silently ignored.
  }
}

function clearDraft(root: string) {
  try {
    localStorage.removeItem(draftKey(root));
  } catch {
    // Same as above: nothing to recover, nothing to break.
  }
}

const SUBJECT_SOFT_LIMIT = 50;
const SUBJECT_HARD_LIMIT = 72;

export const CommitPanel: Component<{
  root: string;
  stagedCount: number;
  onCommit: (message: string, options: CommitOptions) => Promise<void>;
  onCheckHeadPublished: () => Promise<boolean>;
  onAmend: (message: string, options: CommitOptions) => Promise<void>;
  error: string | null;
  hookOutput: string | null;
  messageTemplate: CommitMessageTemplate | null;
}> = (props) => {
  const [message, setMessage] = createSignal("");
  const [author, setAuthor] = createSignal("");
  const [signOff, setSignOff] = createSignal(false);
  const [sign, setSign] = createSignal(false);
  const [amendMode, setAmendMode] = createSignal(false);
  const [committing, setCommitting] = createSignal(false);
  const [publishedWarning, setPublishedWarning] = createSignal(false);
  const [draftExists, setDraftExists] = createSignal(false);

  createEffect(() => {
    const draft = loadDraft(props.root);
    if (draft) {
      setMessage(draft.message);
      setAuthor(draft.author);
      setSignOff(draft.signOff);
      setSign(draft.sign);
      setDraftExists(true);
    } else {
      setMessage("");
      setAuthor("");
      setSignOff(false);
      setSign(false);
      setDraftExists(false);
    }
  });

  createEffect(() => {
    const template = props.messageTemplate;
    if (!draftExists() && template && !message()) {
      setMessage(template.content);
    }
  });

  function persist() {
    saveDraft(props.root, { message: message(), author: author(), signOff: signOff(), sign: sign() });
  }

  function updateMessage(value: string) {
    setMessage(value);
    persist();
  }

  function updateAuthor(value: string) {
    setAuthor(value);
    persist();
  }

  function updateSignOff(value: boolean) {
    setSignOff(value);
    persist();
  }

  function updateSign(value: boolean) {
    setSign(value);
    persist();
  }

  function options(): CommitOptions {
    return { author: author().trim() || undefined, signOff: signOff(), sign: sign() };
  }

  async function doAmend() {
    setCommitting(true);
    try {
      await props.onAmend(message(), options());
      setMessage("");
      clearDraft(props.root);
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
      clearDraft(props.root);
    } finally {
      setCommitting(false);
    }
  }

  const subjectLength = () => message().split("\n")[0].length;

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
        onInput={(e) => updateMessage(e.currentTarget.value)}
      />
      <p
        class="commit-subject-guide"
        classList={{
          "commit-subject-guide-warning": subjectLength() > SUBJECT_SOFT_LIMIT && subjectLength() <= SUBJECT_HARD_LIMIT,
          "commit-subject-guide-danger": subjectLength() > SUBJECT_HARD_LIMIT,
        }}
      >
        Subject: {subjectLength()}/{SUBJECT_SOFT_LIMIT} characters
      </p>
      <input
        class="commit-author-input"
        type="text"
        placeholder="Author override (Name <email>), optional"
        value={author()}
        onInput={(e) => updateAuthor(e.currentTarget.value)}
      />
      <label class="commit-checkbox">
        <input type="checkbox" checked={signOff()} onChange={(e) => updateSignOff(e.currentTarget.checked)} />
        Sign off
      </label>
      <label class="commit-checkbox">
        <input type="checkbox" checked={sign()} onChange={(e) => updateSign(e.currentTarget.checked)} />
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
