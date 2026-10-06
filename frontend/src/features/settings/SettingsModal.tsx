import { type Component, For, Show, createSignal } from "solid-js";
import { type Bookmark, type Settings, type UserThemesResult } from "../../api/types";
import { useModalEscape } from "../../layout/modal-escape";
import { Icon } from "../../ui/Icon";
import { getThemeSwatches } from "../../theme/swatch-theme";
import "./settings.css";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  settings: Settings;
  onSaveSettings: (settings: Settings) => Promise<void>;
  userThemesResult?: UserThemesResult;
  bookmarks: Bookmark[];
  onRemoveBookmark: (root: string) => Promise<void>;
  onOpenBookmark?: (root: string) => void;
}

export const SettingsModal: Component<SettingsModalProps> = (props) => {
  const [activeTab, setActiveTab] = createSignal<"appearance" | "git" | "bookmarks" | "about">("appearance");
  const [currentTheme, setCurrentTheme] = createSignal<string | null>(props.settings.theme);
  const [concurrency, setConcurrency] = createSignal<number>(props.settings.concurrency);
  const [savedMessage, setSavedMessage] = createSignal<string | null>(null);

  useModalEscape(() => props.onClose());

  const handleSelectTheme = async (themeName: string | null) => {
    setCurrentTheme(themeName);
    const updated: Settings = {
      ...props.settings,
      theme: themeName,
      concurrency: concurrency(),
    };
    await props.onSaveSettings(updated);
    showSavedNotification("Theme preference saved");
  };

  const handleConcurrencyChange = async (val: number) => {
    const clamped = Math.max(1, Math.min(32, val));
    setConcurrency(clamped);
    const updated: Settings = {
      ...props.settings,
      theme: currentTheme(),
      concurrency: clamped,
    };
    await props.onSaveSettings(updated);
    showSavedNotification("Network concurrency updated");
  };

  const showSavedNotification = (msg: string) => {
    setSavedMessage(msg);
    setTimeout(() => {
      setSavedMessage(null);
    }, 2500);
  };

  return (
    <Show when={props.isOpen}>
      <div class="settings-modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <div class="settings-modal">
          <header class="settings-modal-header">
            <div class="settings-modal-title" id="settings-title">
              <Icon name="settings" size={20} />
              <span>Preferences</span>
            </div>
            <button
              type="button"
              class="collapse-toggle"
              onClick={props.onClose}
              aria-label="Close preferences"
              title="Close (Esc)"
            >
              <Icon name="close" size={16} />
            </button>
          </header>

          <nav class="settings-modal-tabs" aria-label="Settings categories">
            <button
              type="button"
              class={`settings-tab-btn ${activeTab() === "appearance" ? "active" : ""}`}
              onClick={() => setActiveTab("appearance")}
            >
              <Icon name="palette" size={16} />
              Appearance
            </button>
            <button
              type="button"
              class={`settings-tab-btn ${activeTab() === "git" ? "active" : ""}`}
              onClick={() => setActiveTab("git")}
            >
              <Icon name="branch" size={16} />
              Git & Network
            </button>
            <button
              type="button"
              class={`settings-tab-btn ${activeTab() === "bookmarks" ? "active" : ""}`}
              onClick={() => setActiveTab("bookmarks")}
            >
              <Icon name="folder" size={16} />
              Bookmarks ({props.bookmarks.length})
            </button>
            <button
              type="button"
              class={`settings-tab-btn ${activeTab() === "about" ? "active" : ""}`}
              onClick={() => setActiveTab("about")}
            >
              <Icon name="terminal" size={16} />
              About
            </button>
          </nav>

          <main class="settings-modal-body">
            <Show when={savedMessage()}>
              <div class="user-theme-error-banner" style={{ "border-color": "var(--color-success)", color: "var(--color-success)" }}>
                <Icon name="check" size={16} />
                <span>{savedMessage()}</span>
              </div>
            </Show>

            <Show when={activeTab() === "appearance"}>
              <section class="settings-section">
                <div class="settings-section-title">Color Theme</div>

                <div class="settings-field-row" style={{ "padding-bottom": "var(--space-2)" }}>
                  <div>
                    <div class="settings-field-label">Follow Desktop Environment</div>
                    <div class="settings-field-desc">
                      Automatically sync colors with your desktop theme (Omarchy, GNOME, or system light/dark)
                    </div>
                  </div>
                  <button
                    type="button"
                    class={currentTheme() === null ? "settings-btn-primary" : "settings-btn-secondary"}
                    onClick={() => void handleSelectTheme(null)}
                  >
                    {currentTheme() === null ? "Active (Follow Desktop)" : "Enable Follow Desktop"}
                  </button>
                </div>

                <div class="settings-field-desc">Or choose a standalone theme:</div>

                <div class="settings-theme-grid">
                  <button
                    type="button"
                    class={`settings-theme-card ${currentTheme() === "dark" ? "active" : ""}`}
                    onClick={() => void handleSelectTheme("dark")}
                  >
                    <div class="settings-theme-preview">
                      <For each={getThemeSwatches("dark")}>
                        {(color) => <span class="settings-theme-swatch" style={{ "background-color": color }} />}
                      </For>
                    </div>
                    <span class="settings-field-label">GitTree Dark (Built-in)</span>
                  </button>

                  <button
                    type="button"
                    class={`settings-theme-card ${currentTheme() === "light" ? "active" : ""}`}
                    onClick={() => void handleSelectTheme("light")}
                  >
                    <div class="settings-theme-preview">
                      <For each={getThemeSwatches("light")}>
                        {(color) => <span class="settings-theme-swatch" style={{ "background-color": color }} />}
                      </For>
                    </div>
                    <span class="settings-field-label">GitTree Light (Built-in)</span>
                  </button>

                  <For each={props.userThemesResult?.themes ?? []}>
                    {(t) => (
                      <button
                        type="button"
                        class={`settings-theme-card ${currentTheme() === t.id ? "active" : ""}`}
                        onClick={() => void handleSelectTheme(t.id)}
                      >
                        <div class="settings-theme-preview">
                          <For each={getThemeSwatches(t.id)}>
                            {(color) => <span class="settings-theme-swatch" style={{ "background-color": color }} />}
                          </For>
                        </div>
                        <span class="settings-field-label">{t.name}</span>
                      </button>
                    )}
                  </For>
                </div>
              </section>
            </Show>

            <Show when={activeTab() === "git"}>
              <section class="settings-section">
                <div class="settings-section-title">Network & Performance</div>

                <div class="settings-field">
                  <div class="settings-field-row">
                    <div>
                      <label for="concurrency-input" class="settings-field-label">
                        Submodule Network Concurrency
                      </label>
                      <div class="settings-field-desc">
                        Maximum number of submodule repositories fetched in parallel during background refreshes.
                      </div>
                    </div>
                    <input
                      id="concurrency-input"
                      type="number"
                      min="1"
                      max="32"
                      class="settings-input"
                      style={{ width: "80px", "text-align": "center" }}
                      value={concurrency()}
                      onInput={(e) => void handleConcurrencyChange(parseInt(e.currentTarget.value, 10) || 8)}
                    />
                  </div>
                </div>

                <div class="settings-field" style={{ "margin-top": "var(--space-3)" }}>
                  <div class="settings-field-label">Privacy & Offline Guarantee</div>
                  <div class="settings-field-desc">
                    GitTree never opens outbound analytics, crash report sockets, or telemetry connections.
                    All Git commands execute strictly against your configured repository remotes.
                  </div>
                </div>
              </section>
            </Show>

            <Show when={activeTab() === "bookmarks"}>
              <section class="settings-section">
                <div class="settings-section-title">Saved Bookmarks</div>
                <div class="settings-field-desc">
                  Repositories bookmarked in your workspace. Stored in <code>~/.config/gittree/bookmarks.json</code>.
                </div>

                <Show
                  when={props.bookmarks.length > 0}
                  fallback={<div class="text-muted" style={{ padding: "var(--space-3)" }}>No repositories bookmarked yet.</div>}
                >
                  <div class="settings-bookmark-list">
                    <For each={props.bookmarks}>
                      {(b) => (
                        <div class="settings-bookmark-row">
                          <div
                            style={{ display: "flex", "align-items": "center", gap: "var(--space-2)", overflow: "hidden", cursor: props.onOpenBookmark ? "pointer" : "default" }}
                            onClick={() => props.onOpenBookmark?.(b.root)}
                          >
                            <Icon name="folder" size={14} />
                            <span class="settings-bookmark-path" title={b.root}>
                              {b.root}
                            </span>
                          </div>
                          <button
                            type="button"
                            class="collapse-toggle"
                            title="Remove bookmark"
                            onClick={() => void props.onRemoveBookmark(b.root)}
                          >
                            <Icon name="trash" size={14} />
                          </button>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>
              </section>
            </Show>

            <Show when={activeTab() === "about"}>
              <section class="settings-section">
                <div class="settings-section-title">GitTree 1.0.0</div>
                <div class="settings-field-desc">
                  A high-performance desktop Git client for Linux built around a real-time submodule workspace and the daily repository workflow.
                </div>
                <div class="settings-field-desc">
                  Parity with classic Git desktop loops: line-level staging, interactive blame, visual rebase, and reflog recovery.
                </div>
                <div style={{ "margin-top": "var(--space-3)", display: "flex", gap: "var(--space-2)" }}>
                  <span class="badge-tag">Zero Telemetry</span>
                  <span class="badge-tag">Wayland / X11</span>
                  <span class="badge-tag">Rust & Tauri 2</span>
                </div>
              </section>
            </Show>
          </main>

          <footer class="settings-modal-footer">
            <button type="button" class="settings-btn-primary" onClick={props.onClose}>
              Done
            </button>
          </footer>
        </div>
      </div>
    </Show>
  );
};
