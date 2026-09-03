import { type Component, For, Show } from "solid-js";
import type { LogEntry } from "../../api/types";

export const OperationLogView: Component<{ entries: LogEntry[] }> = (props) => {
  const sorted = () => [...props.entries].reverse();

  return (
    <div class="operation-log">
      <h2 class="operation-log-title">Operation log</h2>
      <Show when={sorted().length > 0} fallback={<p class="text-muted">No commands run yet.</p>}>
        <ul class="operation-log-list">
          <For each={sorted()}>
            {(entry) => (
              <li classList={{ "log-entry": true, "log-entry-write": entry.write, "log-entry-error": entry.exit_status !== 0 }}>
                <code>git {entry.args.join(" ")}</code>
                <span class="text-faint">
                  {entry.duration_ms}ms · exit {entry.exit_status} · {entry.write ? "write" : "read"}
                </span>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
};
