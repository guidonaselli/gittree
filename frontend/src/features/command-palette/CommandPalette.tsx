import {
  type Component,
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import "./command-palette.css";
import { Icon } from "../../ui/Icon";

export interface CommandPaletteItem {
  id: string;
  title: string;
  category: string;
  shortcut?: string;
  available: boolean;
  unavailableReason?: string;
  isDestructive?: boolean;
  onExecute: () => void;
}

export interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  items: CommandPaletteItem[];
}

export const CommandPalette: Component<CommandPaletteProps> = (props) => {
  const [query, setQuery] = createSignal("");
  const [selectedIndex, setSelectedIndex] = createSignal(0);
  let inputRef: HTMLInputElement | undefined;
  let previousActiveElement: HTMLElement | null = null;

  onMount(() => {
    previousActiveElement = document.activeElement as HTMLElement | null;
  });

  createEffect(() => {
    if (props.open) {
      previousActiveElement = document.activeElement as HTMLElement | null;
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef?.focus(), 10);
    } else {
      if (previousActiveElement && typeof previousActiveElement.focus === "function") {
        previousActiveElement.focus();
      }
    }
  });

  const filteredItems = createMemo(() => {
    const q = query().trim().toLowerCase();
    if (!q) return props.items;
    return props.items.filter(
      (item) =>
        item.title.toLowerCase().includes(q) ||
        item.category.toLowerCase().includes(q) ||
        (item.shortcut && item.shortcut.toLowerCase().includes(q))
    );
  });

  createEffect(() => {
    // Keep selected index within bounds
    const max = Math.max(0, filteredItems().length - 1);
    if (selectedIndex() > max) {
      setSelectedIndex(max);
    }
  });

  function executeItem(item: CommandPaletteItem) {
    if (!item.available) return;
    props.onClose();
    item.onExecute();
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      props.onClose();
      return;
    }

    const items = filteredItems();
    if (items.length === 0) return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % items.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev - 1 + items.length) % items.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const current = items[selectedIndex()];
      if (current) executeItem(current);
    }
  }

  onMount(() => {
    const onGlobalKeyDown = (e: KeyboardEvent) => {
      if (props.open && e.key === "Escape") {
        e.preventDefault();
        props.onClose();
      }
    };
    window.addEventListener("keydown", onGlobalKeyDown, true);
    onCleanup(() => window.removeEventListener("keydown", onGlobalKeyDown, true));
  });

  return (
    <Show when={props.open}>
      <div
        class="command-palette-backdrop"
        onClick={(e) => {
          if (e.target === e.currentTarget) props.onClose();
        }}
        role="presentation"
      >
        <div
          class="command-palette-modal"
          role="dialog"
          aria-modal="true"
          aria-label="Command Palette"
          onKeyDown={handleKeyDown}
        >
          <div class="command-palette-search-box">
            <Icon name="search" size={16} class="command-palette-search-icon" />
            <input
              ref={inputRef}
              type="text"
              class="command-palette-input"
              placeholder="Type a command or repository name…"
              value={query()}
              onInput={(e) => {
                setQuery(e.currentTarget.value);
                setSelectedIndex(0);
              }}
              aria-autocomplete="list"
              aria-controls="command-palette-results"
            />
            <button
              type="button"
              class="command-palette-close-btn"
              onClick={props.onClose}
              title="Close (Esc)"
              aria-label="Close"
            >
              <Icon name="close" size={14} />
            </button>
          </div>

          <div
            id="command-palette-results"
            class="command-palette-list"
            role="listbox"
            aria-label="Commands"
          >
            <Show
              when={filteredItems().length > 0}
              fallback={
                <div class="command-palette-empty">No matching commands or repositories found.</div>
              }
            >
              <For each={filteredItems()}>
                {(item, index) => {
                  const isSelected = () => index() === selectedIndex();
                  return (
                    <div
                      class="command-palette-item"
                      classList={{
                        "item-selected": isSelected(),
                        "item-disabled": !item.available,
                        "item-destructive": !!item.isDestructive,
                      }}
                      role="option"
                      aria-selected={isSelected()}
                      aria-disabled={!item.available}
                      onClick={() => executeItem(item)}
                      onMouseEnter={() => setSelectedIndex(index())}
                    >
                      <div class="command-item-main">
                        <span class="command-item-category">{item.category}</span>
                        <span class="command-item-title">{item.title}</span>
                      </div>

                      <div class="command-item-meta">
                        <Show when={!item.available && item.unavailableReason}>
                          <span
                            class="command-unavailable-badge"
                            title={item.unavailableReason}
                          >
                            Unavailable: {item.unavailableReason}
                          </span>
                        </Show>
                        <Show when={item.shortcut}>
                          <kbd class="command-shortcut-kbd">{item.shortcut}</kbd>
                        </Show>
                      </div>
                    </div>
                  );
                }}
              </For>
            </Show>
          </div>

          <div class="command-palette-footer">
            <span class="command-palette-hint">
              <kbd>↑</kbd> <kbd>↓</kbd> to navigate
            </span>
            <span class="command-palette-hint">
              <kbd>Enter</kbd> to select
            </span>
            <span class="command-palette-hint">
              <kbd>Esc</kbd> to close
            </span>
          </div>
        </div>
      </div>
    </Show>
  );
};
