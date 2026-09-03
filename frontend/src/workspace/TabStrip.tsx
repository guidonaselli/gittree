import { type Component, For, Show } from "solid-js";
import type { Tab } from "./model";

export const TabStrip: Component<{
  tabs: Tab[];
  activeTabId: string;
  onSelect: (tabId: string) => void;
  onClose: (tabId: string) => void;
}> = (props) => {
  return (
    <div class="tab-strip" role="tablist" aria-label="Open tabs">
      <For each={props.tabs}>
        {(tab, index) => (
          <div classList={{ "tab-chip": true, "tab-chip-active": tab.id === props.activeTabId }}>
            <button role="tab" aria-selected={tab.id === props.activeTabId} class="tab-chip-label" onClick={() => props.onSelect(tab.id)}>
              {tab.label}
            </button>
            <Show when={index() > 0}>
              <button class="tab-chip-close" aria-label={`Close ${tab.label}`} onClick={() => props.onClose(tab.id)}>
                ×
              </button>
            </Show>
          </div>
        )}
      </For>
    </div>
  );
};
