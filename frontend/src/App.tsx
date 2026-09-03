import { type Component, For, Show, createResource, createSignal } from "solid-js";
import { getRepositoryState, getSubmoduleMatrix, resolveRepositoryRoot } from "./api/commands";
import { RepositoryStatus } from "./features/repository/RepositoryStatus";
import { SubmoduleMatrix } from "./features/submodules/SubmoduleMatrix";
import {
  useDetailPanelCollapsed,
  useDetailPanelWidth,
  useSidebarCollapsed,
  useSidebarWidth,
} from "./layout/persisted-layout";
import { Resizer } from "./layout/Resizer";
import "./layout/layout.css";
import "./features/repository/repository.css";
import "./features/submodules/submodule-matrix.css";

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

const RECENT_KEY = "gittree.recentRoots";

function readRecent(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY);
    return raw ? (JSON.parse(raw) as string[]) : [];
  } catch {
    return [];
  }
}

function pushRecent(root: string) {
  const next = [root, ...readRecent().filter((r) => r !== root)].slice(0, 10);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    // best-effort; bookmarks are a convenience, not a hard dependency
  }
}

export const App: Component = () => {
  const [sidebarWidth, setSidebarWidth] = useSidebarWidth();
  const [sidebarCollapsed, setSidebarCollapsed] = useSidebarCollapsed();
  const [detailWidth, setDetailWidth] = useDetailPanelWidth();
  const [detailCollapsed, setDetailCollapsed] = useDetailPanelCollapsed();

  const [pathInput, setPathInput] = createSignal("");
  const [activeRoot, setActiveRoot] = createSignal<string | null>(null);
  const [openError, setOpenError] = createSignal<string | null>(null);
  const [recent, setRecent] = createSignal<string[]>(readRecent());

  const [repoState, { refetch: refetchRepoState }] = createResource(activeRoot, (root) => getRepositoryState(root));
  const [submodules, { refetch: refetchSubmodules }] = createResource(activeRoot, (root) => getSubmoduleMatrix(root));

  async function openPath(path: string) {
    setOpenError(null);
    try {
      const root = await resolveRepositoryRoot(path);
      setActiveRoot(root);
      pushRecent(root);
      setRecent(readRecent());
    } catch (e) {
      setOpenError(String(e));
    }
  }

  function refreshAll() {
    refetchRepoState();
    refetchSubmodules();
  }

  return (
    <div class="app-shell">
      <div class="app-titlebar">
        <span>GitTree</span>
        <Show when={activeRoot()}>
          <span class="text-muted">— {activeRoot()}</span>
          <button class="collapse-toggle" onClick={refreshAll}>
            Refresh
          </button>
          <button
            class="collapse-toggle"
            aria-expanded={!detailCollapsed()}
            onClick={() => setDetailCollapsed(!detailCollapsed())}
          >
            {detailCollapsed() ? "Show detail panel" : "Hide detail panel"}
          </button>
        </Show>
      </div>

      <div class="app-body">
        <Show when={!sidebarCollapsed()}>
          <nav
            class="pane-sidebar"
            style={{ width: `${clamp(sidebarWidth(), 180, 480)}px` }}
            aria-label="Repository bookmarks"
          >
            <div class="repo-status" style={{ "flex-direction": "column", "align-items": "stretch" }}>
              <label for="open-path-input">Open repository</label>
              <input
                id="open-path-input"
                type="text"
                placeholder="/path/to/repository"
                value={pathInput()}
                onInput={(e) => setPathInput(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void openPath(pathInput());
                }}
              />
              <button onClick={() => void openPath(pathInput())}>Open</button>
              <Show when={openError()}>
                <div class="text-danger">{openError()}</div>
              </Show>
            </div>
            <ul>
              <For each={recent()}>
                {(root) => (
                  <li>
                    <button class="collapse-toggle" onClick={() => void openPath(root)}>
                      {root}
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </nav>
          <Resizer label="Resize sidebar" onResize={(d) => setSidebarWidth(clamp(sidebarWidth() + d, 180, 480))} />
        </Show>

        <main class="pane-content">
          <button
            class="collapse-toggle"
            aria-expanded={!sidebarCollapsed()}
            onClick={() => setSidebarCollapsed(!sidebarCollapsed())}
          >
            {sidebarCollapsed() ? "Show sidebar" : "Hide sidebar"}
          </button>

          <Show when={activeRoot()} fallback={<p class="text-muted">Open a repository to begin.</p>}>
            <Show when={repoState()}>{(state) => <RepositoryStatus state={state()} />}</Show>
            <Show when={submodules() && submodules()!.length > 0}>
              <SubmoduleMatrix submodules={submodules()!} />
            </Show>
            <Show when={submodules.loading}>
              <p class="text-muted">Loading submodules…</p>
            </Show>
          </Show>
        </main>

        <Show when={!detailCollapsed() && activeRoot()}>
          <Resizer label="Resize detail panel" onResize={(d) => setDetailWidth(clamp(detailWidth() - d, 240, 640))} />
          <aside class="pane-detail" style={{ width: `${clamp(detailWidth(), 240, 640)}px` }} aria-label="Detail panel">
            <p class="text-muted" style={{ padding: "var(--space-3)" }}>
              Detail panel — commit detail, diff and blame land here (F-002/F-003).
            </p>
          </aside>
        </Show>
      </div>
    </div>
  );
};
