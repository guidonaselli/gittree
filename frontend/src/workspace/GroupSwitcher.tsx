import { type Component, For } from "solid-js";
import type { WorkspaceGroup } from "./model";

export const GroupSwitcher: Component<{
  groups: WorkspaceGroup[];
  activeGroupId: string | null;
  onSelect: (groupId: string) => void;
  onClose: (groupId: string) => void;
}> = (props) => {
  return (
    <div class="group-switcher" role="tablist" aria-label="Open repositories">
      <For each={props.groups}>
        {(group) => (
          <div classList={{ "group-chip": true, "group-chip-active": group.id === props.activeGroupId }}>
            <button
              role="tab"
              aria-selected={group.id === props.activeGroupId}
              class="group-chip-label"
              onClick={() => props.onSelect(group.id)}
              title={group.rootPath}
            >
              {group.rootLabel}
            </button>
            <button class="group-chip-close" aria-label={`Close ${group.rootLabel}`} onClick={() => props.onClose(group.id)}>
              ×
            </button>
          </div>
        )}
      </For>
    </div>
  );
};
