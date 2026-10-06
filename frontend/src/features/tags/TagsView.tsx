import { type Component, For, Show, createMemo, createResource, createSignal } from "solid-js";
import {
  checkoutBranch,
  createTag,
  deleteRemoteTag,
  deleteTag,
  getTags,
  pushTag,
} from "../../api/commands";
import type { TagEntry } from "../../api/types";
import "./tags.css";
import { Icon } from "../../ui/Icon";

export interface TagsViewProps {
  root: string;
  onCheckoutSuccess?: (ref: string) => void;
}

export const TagsView: Component<TagsViewProps> = (props) => {
  const [tags, { refetch: refetchTags }] = createResource(
    () => props.root,
    (root) => getTags(root)
  );

  const [filter, setFilter] = createSignal("");
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [actionNotice, setActionNotice] = createSignal<string | null>(null);

  // Modal dialog states
  const [showCreateModal, setShowCreateModal] = createSignal(false);
  const [pushTarget, setPushTarget] = createSignal<TagEntry | null>(null);
  const [deleteLocalTarget, setDeleteLocalTarget] = createSignal<TagEntry | null>(null);
  const [deleteRemoteTarget, setDeleteRemoteTarget] = createSignal<TagEntry | null>(null);

  const filteredTags = createMemo(() => {
    const list = tags() ?? [];
    const query = filter().toLowerCase().trim();
    if (!query) return list;
    return list.filter(
      (t) =>
        t.name.toLowerCase().includes(query) ||
        t.target_commit.toLowerCase().includes(query) ||
        (t.subject && t.subject.toLowerCase().includes(query)) ||
        (t.commit_subject && t.commit_subject.toLowerCase().includes(query)) ||
        (t.tagger_name && t.tagger_name.toLowerCase().includes(query))
    );
  });

  return (
    <div class="tags-view">
      <div class="tags-toolbar">
        <input
          type="text"
          class="tags-filter-input"
          placeholder="Filter tags..."
          value={filter()}
          onInput={(e) => setFilter(e.currentTarget.value)}
        />
        <button class="tags-btn tags-btn-primary" onClick={() => setShowCreateModal(true)}>
          New tag
        </button>
        <button class="tags-btn" onClick={() => refetchTags()}>
          Refresh
        </button>
      </div>

      <Show when={errorMessage()}>
        <div class="tag-banner-error">{errorMessage()}</div>
      </Show>

      <Show when={actionNotice()}>
        <div class="tag-banner-notice">{actionNotice()}</div>
      </Show>

      <div class="tags-group">
        <ul class="tags-list">
          <For each={filteredTags()} fallback={<li class="text-muted">No tags found.</li>}>
            {(tag) => (
              <li class="tag-item">
                <div class="tag-item-header">
                  <div class="tag-item-main">
                    <span class="tag-item-name">{tag.name}</span>
                    <span class={tag.is_annotated ? "tag-badge-annotated" : "tag-badge-lightweight"}>
                      {tag.is_annotated ? "annotated" : "lightweight"}
                    </span>
                    <span class="tag-item-commit">{tag.target_commit}</span>
                    <span class="tag-item-subject" title={tag.commit_subject ?? ""}>
                      {tag.commit_subject ?? ""}
                    </span>
                  </div>
                  <div class="tag-item-actions">
                    <button
                      class="tags-btn"
                      onClick={async () => {
                        try {
                          await checkoutBranch(props.root, tag.name);
                          props.onCheckoutSuccess?.(tag.name);
                        } catch (e) {
                          setErrorMessage(String(e));
                        }
                      }}
                      title="Checkout commit at this tag"
                    >
                      Checkout
                    </button>
                    <button class="tags-btn" onClick={() => setPushTarget(tag)}>
                      Push to remote...
                    </button>
                    <button class="tags-btn" onClick={() => setDeleteRemoteTarget(tag)}>
                      Delete on remote...
                    </button>
                    <button
                      class="tags-btn tags-btn-danger"
                      onClick={() => setDeleteLocalTarget(tag)}
                    >
                      Delete
                    </button>
                  </div>
                </div>

                <Show when={tag.is_annotated}>
                  <div class="tag-item-details">
                    <div>
                      <strong>Tagger:</strong> {tag.tagger_name ?? "Unknown"}{" "}
                      <Show when={tag.tagger_email}>
                        <span>&lt;{tag.tagger_email}&gt;</span>
                      </Show>
                      <Show when={tag.tagger_date}>
                        <span> on {tag.tagger_date}</span>
                      </Show>
                    </div>
                    <Show when={tag.message}>
                      <div class="tag-message-box">{tag.message}</div>
                    </Show>
                  </div>
                </Show>
              </li>
            )}
          </For>
        </ul>
      </div>

      {/* Create Tag Modal */}
      <Show when={showCreateModal()}>
        <CreateTagDialog
          root={props.root}
          onClose={() => setShowCreateModal(false)}
          onSuccess={(name) => {
            setShowCreateModal(false);
            refetchTags();
            setActionNotice(`Tag '${name}' created successfully.`);
            setTimeout(() => setActionNotice(null), 5000);
          }}
        />
      </Show>

      {/* Push Tag Modal */}
      <Show when={pushTarget()}>
        {(tag) => (
          <PushTagDialog
            root={props.root}
            tag={tag()}
            onClose={() => setPushTarget(null)}
            onSuccess={(remote) => {
              const name = tag().name;
              setPushTarget(null);
              setActionNotice(`Tag '${name}' pushed to remote '${remote}'.`);
              setTimeout(() => setActionNotice(null), 5000);
            }}
          />
        )}
      </Show>

      {/* Delete Local Tag Dialog */}
      <Show when={deleteLocalTarget()}>
        {(tag) => (
          <DeleteLocalTagDialog
            root={props.root}
            tag={tag()}
            onClose={() => setDeleteLocalTarget(null)}
            onSuccess={() => {
              const name = tag().name;
              setDeleteLocalTarget(null);
              refetchTags();
              setActionNotice(`Local tag '${name}' deleted.`);
              setTimeout(() => setActionNotice(null), 5000);
            }}
          />
        )}
      </Show>

      {/* Delete Remote Tag Dialog */}
      <Show when={deleteRemoteTarget()}>
        {(tag) => (
          <DeleteRemoteTagDialog
            root={props.root}
            tag={tag()}
            onClose={() => setDeleteRemoteTarget(null)}
            onSuccess={(remote) => {
              const name = tag().name;
              setDeleteRemoteTarget(null);
              setActionNotice(`Remote tag '${name}' deleted from '${remote}'.`);
              setTimeout(() => setActionNotice(null), 5000);
            }}
          />
        )}
      </Show>
    </div>
  );
};

interface CreateTagDialogProps {
  root: string;
  onClose: () => void;
  onSuccess: (name: string) => void;
}

const CreateTagDialog: Component<CreateTagDialogProps> = (props) => {
  const [name, setName] = createSignal("");
  const [targetRef, setTargetRef] = createSignal("");
  const [isAnnotated, setIsAnnotated] = createSignal(true);
  const [message, setMessage] = createSignal("");
  const [force, setForce] = createSignal(false);
  const [pushRemote, setPushRemote] = createSignal(false);
  const [remoteName, setRemoteName] = createSignal("origin");
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function handleCreate() {
    const trimmed = name().trim();
    if (!trimmed) {
      setError("Tag name cannot be empty.");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      await createTag(props.root, {
        name: trimmed,
        target_ref: targetRef().trim() || null,
        message: isAnnotated() ? message().trim() || trimmed : null,
        force: force(),
      });

      if (pushRemote()) {
        await pushTag(props.root, {
          remote: remoteName().trim() || "origin",
          name: trimmed,
          force: force(),
        });
      }

      props.onSuccess(trimmed);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="tag-modal-overlay">
      <div class="tag-modal-card">
        <div class="tag-modal-header">
          <h3>Create New Tag</h3>
          <button class="tags-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="tag-modal-body">
          <Show when={error()}>
            <div class="tag-banner-error">{error()}</div>
          </Show>

          <div class="tag-modal-field">
            <label for="new-tag-name">Tag name</label>
            <input
              id="new-tag-name"
              type="text"
              placeholder="e.g. v1.0.0"
              value={name()}
              onInput={(e) => setName(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !isAnnotated()) void handleCreate();
              }}
            />
          </div>

          <div class="tag-modal-field">
            <label for="new-tag-target">Target commit or branch (optional, defaults to HEAD)</label>
            <input
              id="new-tag-target"
              type="text"
              placeholder="HEAD"
              value={targetRef()}
              onInput={(e) => setTargetRef(e.currentTarget.value)}
            />
          </div>

          <div class="tag-modal-field">
            <label>Tag type</label>
            <div class="tag-modal-radio-group">
              <label class="tag-modal-radio-label">
                <input
                  type="radio"
                  name="tag-type"
                  checked={isAnnotated()}
                  onChange={() => setIsAnnotated(true)}
                />
                Annotated tag
              </label>
              <label class="tag-modal-radio-label">
                <input
                  type="radio"
                  name="tag-type"
                  checked={!isAnnotated()}
                  onChange={() => setIsAnnotated(false)}
                />
                Lightweight tag
              </label>
            </div>
          </div>

          <Show when={isAnnotated()}>
            <div class="tag-modal-field">
              <label for="new-tag-message">Tag message</label>
              <textarea
                id="new-tag-message"
                placeholder="Release notes or description..."
                value={message()}
                onInput={(e) => setMessage(e.currentTarget.value)}
              />
            </div>
          </Show>

          <label class="tag-modal-checkbox">
            <input
              type="checkbox"
              checked={force()}
              onChange={(e) => setForce(e.currentTarget.checked)}
            />
            <span>Force replace if tag already exists (-f)</span>
          </label>

          <label class="tag-modal-checkbox">
            <input
              type="checkbox"
              checked={pushRemote()}
              onChange={(e) => setPushRemote(e.currentTarget.checked)}
            />
            <span>Push immediately to remote:</span>
          </label>

          <Show when={pushRemote()}>
            <div class="tag-modal-field" style={{ "margin-left": "var(--space-4)" }}>
              <input
                type="text"
                placeholder="origin"
                value={remoteName()}
                onInput={(e) => setRemoteName(e.currentTarget.value)}
              />
            </div>
          </Show>
        </div>
        <div class="tag-modal-footer">
          <button
            class="tags-btn tags-btn-primary"
            disabled={loading() || !name().trim()}
            onClick={() => void handleCreate()}
          >
            {loading() ? "Creating..." : "Create Tag"}
          </button>
          <button class="tags-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

interface PushTagDialogProps {
  root: string;
  tag: TagEntry;
  onClose: () => void;
  onSuccess: (remote: string) => void;
}

const PushTagDialog: Component<PushTagDialogProps> = (props) => {
  const [remote, setRemote] = createSignal("origin");
  const [force, setForce] = createSignal(false);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function handlePush() {
    setLoading(true);
    setError(null);
    try {
      await pushTag(props.root, {
        remote: remote().trim() || "origin",
        name: props.tag.name,
        force: force(),
      });
      props.onSuccess(remote().trim() || "origin");
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="tag-modal-overlay">
      <div class="tag-modal-card">
        <div class="tag-modal-header">
          <h3>Push Tag '{props.tag.name}' to Remote</h3>
          <button class="tags-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="tag-modal-body">
          <Show when={error()}>
            <div class="tag-banner-error">{error()}</div>
          </Show>

          <p>
            Push tag <strong style={{ color: "var(--color-accent)" }}>{props.tag.name}</strong> (pointing to{" "}
            <code>{props.tag.target_commit}</code>) to a remote repository.
          </p>

          <div class="tag-modal-field">
            <label for="push-remote-name">Remote name</label>
            <input
              id="push-remote-name"
              type="text"
              value={remote()}
              onInput={(e) => setRemote(e.currentTarget.value)}
            />
          </div>

          <label class="tag-modal-checkbox">
            <input
              type="checkbox"
              checked={force()}
              onChange={(e) => setForce(e.currentTarget.checked)}
            />
            <span>Force push (--force)</span>
          </label>
        </div>
        <div class="tag-modal-footer">
          <button
            class="tags-btn tags-btn-primary"
            disabled={loading()}
            onClick={() => void handlePush()}
          >
            {loading() ? "Pushing..." : "Push Tag"}
          </button>
          <button class="tags-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

interface DeleteLocalTagDialogProps {
  root: string;
  tag: TagEntry;
  onClose: () => void;
  onSuccess: () => void;
}

const DeleteLocalTagDialog: Component<DeleteLocalTagDialogProps> = (props) => {
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function handleDelete() {
    setLoading(true);
    setError(null);
    try {
      await deleteTag(props.root, props.tag.name);
      props.onSuccess();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="tag-modal-overlay">
      <div class="tag-modal-card">
        <div class="tag-modal-header">
          <h3>Delete Local Tag</h3>
          <button class="tags-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="tag-modal-body">
          <Show when={error()}>
            <div class="tag-banner-error">{error()}</div>
          </Show>

          <p>
            Are you sure you want to delete the local tag{" "}
            <strong style={{ color: "var(--color-danger)" }}>{props.tag.name}</strong>?
          </p>
          <p class="text-muted">
            This tag points to commit <code>{props.tag.target_commit}</code>. The commit itself will
            not be deleted.
          </p>
        </div>
        <div class="tag-modal-footer">
          <button
            class="tags-btn tags-btn-danger"
            disabled={loading()}
            onClick={() => void handleDelete()}
          >
            {loading() ? "Deleting..." : "Delete Tag"}
          </button>
          <button class="tags-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

interface DeleteRemoteTagDialogProps {
  root: string;
  tag: TagEntry;
  onClose: () => void;
  onSuccess: (remote: string) => void;
}

const DeleteRemoteTagDialog: Component<DeleteRemoteTagDialogProps> = (props) => {
  const [remote, setRemote] = createSignal("origin");
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function handleDelete() {
    setLoading(true);
    setError(null);
    try {
      await deleteRemoteTag(props.root, remote().trim() || "origin", props.tag.name);
      props.onSuccess(remote().trim() || "origin");
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="tag-modal-overlay">
      <div class="tag-modal-card">
        <div class="tag-modal-header">
          <h3>Delete Remote Tag</h3>
          <button class="tags-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="tag-modal-body">
          <Show when={error()}>
            <div class="tag-banner-error">{error()}</div>
          </Show>

          <p>
            Permanently delete tag{" "}
            <strong style={{ color: "var(--color-danger)" }}>{props.tag.name}</strong> from the remote
            server?
          </p>

          <div class="tag-modal-field">
            <label for="del-remote-name">Remote name</label>
            <input
              id="del-remote-name"
              type="text"
              value={remote()}
              onInput={(e) => setRemote(e.currentTarget.value)}
            />
          </div>

          <p class="text-muted">
            This runs <code>git push &lt;remote&gt; --delete refs/tags/{props.tag.name}</code>. Other
            collaborators may still have local copies until pruned.
          </p>
        </div>
        <div class="tag-modal-footer">
          <button
            class="tags-btn tags-btn-danger"
            disabled={loading()}
            onClick={() => void handleDelete()}
          >
            {loading() ? "Deleting on Remote..." : "Delete on Remote"}
          </button>
          <button class="tags-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};
