import { Show, type Component } from "solid-js";
import "./sync.css";
import { Icon } from "../../ui/Icon";

export interface SyncToolbarProps {
  onOpenFetch: () => void;
  onOpenPull: () => void;
  onOpenPush: () => void;
  aheadCount?: number;
  behindCount?: number;
  isSyncing?: boolean;
  onCancelSync?: () => void;
}

export const SyncToolbar: Component<SyncToolbarProps> = (props) => {
  return (
    <div class="sync-toolbar-actions">
      <button
        class="sync-toolbar-btn"
        onClick={props.onOpenFetch}
        disabled={props.isSyncing}
        title="Fetch from remotes (all or specific remote, prune, tags)"
      >
        <Icon name="refresh" size={12} />
        <span>Fetch</span>
      </button>

      <button
        class="sync-toolbar-btn"
        onClick={props.onOpenPull}
        disabled={props.isSyncing}
        title="Pull changes from remote (rebase, merge, fast-forward)"
      >
        <Icon name="arrow-down" size={12} />
        <span>Pull</span>
        <Show when={(props.behindCount ?? 0) > 0}>
          <span class="sync-badge-behind">
            <span>{props.behindCount}</span>
          </span>
        </Show>
      </button>

      <button
        class="sync-toolbar-btn"
        onClick={props.onOpenPush}
        disabled={props.isSyncing}
        title="Push local commits to remote (safe, force-with-lease, bare force)"
      >
        <Icon name="arrow-up" size={12} />
        <span>Push</span>
        <Show when={(props.aheadCount ?? 0) > 0}>
          <span class="sync-badge-ahead">
            <span>{props.aheadCount}</span>
          </span>
        </Show>
      </button>

      <Show when={props.isSyncing}>
        <div style={{ display: "flex", "align-items": "center", gap: "var(--space-1)" }}>
          <span class="text-muted" style={{ "font-size": "var(--font-size-xs)" }}>
            Syncing...
          </span>
          <Show when={props.onCancelSync}>
            <button
              class="collapse-toggle text-danger"
              style={{ "font-size": "var(--font-size-xs)", padding: "0 var(--space-1)" }}
              onClick={props.onCancelSync}
              title="Cancel in-flight network operation (leaves repository unmodified)"
            >
              Cancel
            </button>
          </Show>
        </div>
      </Show>
    </div>
  );
};
