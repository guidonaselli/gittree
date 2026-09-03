import { createSignal, type Accessor, type Setter } from "solid-js";

/**
 * Per-viewer layout state (pane sizes, collapse state, active view) kept in
 * the webview's own localStorage. This is intentionally separate from the
 * cross-repository application settings file (task 1.14, XDG config,
 * Rust-owned) — layout is UI-shell state, not a Git or repository setting,
 * and reading/writing it must never touch the process layer.
 *
 * Spec: ui-shell-and-theming, "Application layout" — "layout, pane sizes and
 * per-repository active view MUST persist across restarts."
 */
function persistedSignal<T>(key: string, initial: T): [Accessor<T>, Setter<T>] {
  let stored: T = initial;
  try {
    const raw = localStorage.getItem(key);
    if (raw !== null) stored = JSON.parse(raw) as T;
  } catch {
    // A corrupt or foreign value falls back to the default rather than
    // throwing — layout state is a convenience, never a hard dependency.
    stored = initial;
  }
  const [get, set] = createSignal<T>(stored);
  const wrappedSet = ((next: unknown) => {
    const value = set(next as never);
    try {
      localStorage.setItem(key, JSON.stringify(get()));
    } catch {
      // Storage can be unavailable (private mode, quota); layout simply
      // stops persisting rather than breaking the app.
    }
    return value;
  }) as Setter<T>;
  return [get, wrappedSet];
}

export function useSidebarWidth() {
  return persistedSignal<number>("gittree.layout.sidebarWidth", 260);
}

export function useDetailPanelWidth() {
  return persistedSignal<number>("gittree.layout.detailPanelWidth", 360);
}

export function useSidebarCollapsed() {
  return persistedSignal<boolean>("gittree.layout.sidebarCollapsed", false);
}

export function useDetailPanelCollapsed() {
  return persistedSignal<boolean>("gittree.layout.detailPanelCollapsed", false);
}
