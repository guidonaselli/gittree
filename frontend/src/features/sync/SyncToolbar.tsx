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
        class="collapse-toggle"
        onClick={props.onOpenFetch}
        disabled={props.isSyncing}
        title="Fetch from remotes (all or specific remote, prune, tags)"
      >
        Fetch
      </button>

      <button
        class="collapse-toggle"
        onClick={props.onOpenPull}
        disabled={props.isSyncing}
        title="Pull changes from remote (rebase, merge, fast-forward)"
      >
        Pull
        <Show when={(props.behindCount ?? 0) > 0}>
          <span class="sync-badge-ahead" style={{ "margin-left": "var(--space-1)", display: "inline-flex", "align-items": "center", gap: "var(--space-1)" }}>
            <Icon name="arrow-down" size={10} />
            <span>{props.behindCount}</span>
          </span>
        </Show>
      </button>

      <button
        class="collapse-toggle"
        onClick={props.onOpenPush}
        disabled={props.isSyncing}
        title="Push local commits to remote (safe, force-with-lease, bare force)"
      >
        Push
        <Show when={(props.aheadCount ?? 0) > 0}>
          <span class="sync-badge-ahead" style={{ "margin-left": "var(--space-1)", display: "inline-flex", "align-items": "center", gap: "var(--space-1)" }}>
            <Icon name="arrow-up" size={10} />
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
