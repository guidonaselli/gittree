import { type Component, For, Show, createResource } from "solid-js";
import { getFileAtRevision } from "../../api/commands";
import { highlightLine } from "../../diff/highlight";

function isImageExtension(path: string): boolean {
  return /\.(png|jpe?g|gif|webp|svg|bmp|ico)$/i.test(path);
}

function imageMime(path: string): string {
  if (/\.svg$/i.test(path)) return "image/svg+xml";
  if (/\.gif$/i.test(path)) return "image/gif";
  if (/\.webp$/i.test(path)) return "image/webp";
  if (/\.bmp$/i.test(path)) return "image/bmp";
  if (/\.ico$/i.test(path)) return "image/x-icon";
  if (/\.jpe?g$/i.test(path)) return "image/jpeg";
  return "image/png";
}

export const HistoricalFileView: Component<{
  root: string;
  rev: string;
  path: string;
  onClose: () => void;
}> = (props) => {
  const [file] = createResource(
    () => [props.root, props.rev, props.path] as const,
    ([root, rev, path]) => getFileAtRevision(root, rev, path),
  );

  const lines = () => (file()?.content ? file()!.content.split("\n") : []);

  async function copyContent() {
    if (file()?.content && !file()?.is_binary) {
      await navigator.clipboard.writeText(file()!.content);
    }
  }

  return (
    <div class="historical-file-view">
      <div class="historical-file-header">
        <div class="historical-file-title">
          <span class="historical-file-name">{props.path}</span>
          <span class="historical-file-rev">@{props.rev.slice(0, 7)}</span>
          <span class="badge">read-only</span>
        </div>
        <div class="historical-file-actions">
          <Show when={file() && !file()!.is_binary}>
            <button class="collapse-toggle" onClick={() => void copyContent()}>
              Copy
            </button>
          </Show>
          <button class="collapse-toggle" onClick={props.onClose}>
            Close
          </button>
        </div>
      </div>
      <Show when={file.error}>
        <p class="diff-apply-error">{String(file.error)}</p>
      </Show>
      <Show when={file.loading}>
        <p class="text-muted">Loading {props.path}@{props.rev.slice(0, 7)}…</p>
      </Show>
      <Show when={file()}>
        {(f) => (
          <Show
            when={!f().is_binary}
            fallback={
              <div class="historical-file-binary">
                <Show
                  when={isImageExtension(props.path)}
                  fallback={<p class="text-muted">Binary file ({f().size} bytes)</p>}
                >
                  <img
                    class="historical-file-image"
                    src={`data:${imageMime(props.path)};base64,${f().content}`}
                    alt={props.path}
                  />
                </Show>
              </div>
            }
          >
            <div class="historical-file-content">
              <pre class="historical-file-pre">
                <code>
                  <For each={lines()}>
                    {(line, i) => (
                      <div class="historical-file-line">
                        <span class="historical-file-lineno">{i() + 1}</span>
                        <span class="historical-file-text">
                          <For each={highlightLine(line)}>
                            {(tok) => (
                              <Show when={tok.class} fallback={tok.text}>
                                <span class={tok.class!}>{tok.text}</span>
                              </Show>
                            )}
                          </For>
                        </span>
                      </div>
                    )}
                  </For>
                </code>
              </pre>
            </div>
          </Show>
        )}
      </Show>
    </div>
  );
};
