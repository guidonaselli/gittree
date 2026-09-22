import { type Component, createSignal, For, Show } from "solid-js";
import type { LogEntry } from "../../api/types";

export const OperationLogView: Component<{ entries: LogEntry[] }> = (props) => {
  const [expandedIndex, setExpandedIndex] = createSignal<number | null>(null);
  const sorted = () => [...props.entries].reverse();

  function toggleExpand(index: number) {
    setExpandedIndex((prev) => (prev === index ? null : index));
  }

  return (
    <div class="operation-log" role="region" aria-label="Operation Log">
      <h2 class="operation-log-title">Operation log</h2>
      <Show
        when={sorted().length > 0}
        fallback={<p class="text-muted">No commands run yet.</p>}
      >
        <ul class="operation-log-list" role="list">
          <For each={sorted()}>
            {(entry, index) => {
              const isError = () => entry.exit_status !== 0;
              const isExpanded = () => expandedIndex() === index();
              const fullCommandLine = () => `git ${entry.args.join(" ")}`;

              return (
                <li
                  classList={{
                    "log-entry": true,
                    "log-entry-write": entry.write,
                    "log-entry-error": isError(),
                  }}
                  role="listitem"
                >
                  <div
                    class="log-entry-header"
                    role="button"
                    tabIndex={0}
                    aria-expanded={isExpanded()}
                    onClick={() => toggleExpand(index())}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        toggleExpand(index());
                      }
                    }}
                  >
                    <code class="log-entry-code">{fullCommandLine()}</code>
                    <div class="log-entry-meta">
                      <span classList={{ "log-exit-status-error": isError() }}>
                        exit {entry.exit_status}
                      </span>
                      <span>·</span>
                      <span>{entry.duration_ms}ms</span>
                      <span>·</span>
                      <span>{entry.write ? "write" : "read"}</span>
                      <span>{isExpanded() ? "▲" : "▼"}</span>
                    </div>
                  </div>

                  <Show when={isExpanded()}>
                    <div class="log-entry-details">
                      <div class="log-verbatim-label">Command Line (Verbatim)</div>
                      <pre class="log-verbatim-block">{fullCommandLine()}</pre>

                      <div class="log-verbatim-label">Exit Status</div>
                      <div classList={{ "log-exit-status-error": isError() }}>
                        {entry.exit_status} ({isError() ? "Failed" : "Success"})
                      </div>

                      <Show when={entry.stderr && entry.stderr.trim().length > 0}>
                        <div class="log-verbatim-label">Standard Error (Verbatim)</div>
                        <pre class="log-verbatim-block log-stderr-block">
                          {entry.stderr}
                        </pre>
                      </Show>
                    </div>
                  </Show>
                </li>
              );
            }}
          </For>
        </ul>
      </Show>
    </div>
  );
};
