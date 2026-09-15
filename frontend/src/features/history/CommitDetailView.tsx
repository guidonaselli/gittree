import { type Component, For, Show, createEffect, createResource, createSignal } from "solid-js";
import { getCommitDetail, type SignatureState } from "../../api/commands";

function signatureLabel(state: SignatureState): { text: string; class: string } {
  if (state === "Unsigned") return { text: "Not signed", class: "signature-unsigned" };
  if ("Valid" in state) return { text: `Valid signature (${state.Valid.signer})`, class: "signature-valid" };
  if ("Invalid" in state) return { text: `Invalid signature: ${state.Invalid.reason}`, class: "signature-invalid" };
  return { text: `Signature unverifiable: ${state.Unverifiable.reason}`, class: "signature-unverifiable" };
}

export const CommitDetailView: Component<{
  root: string;
  sha: string;
  onSelectFileHistory?: (path: string) => void;
  onOpenBlame?: (path: string, rev: string) => void;
  onViewRevision?: (path: string, rev: string) => void;
}> = (props) => {
  const [parentIndex, setParentIndex] = createSignal<number | undefined>(undefined);
  const [detail] = createResource(
    () => [props.root, props.sha, parentIndex()] as const,
    ([root, sha, index]) => getCommitDetail(root, sha, index),
  );

  createEffect(() => {
    void props.sha;
    setParentIndex(undefined);
  });

  return (
    <div class="commit-detail">
      <Show when={detail.error}>
        <p class="diff-apply-error">{String(detail.error)}</p>
      </Show>
      <Show when={detail()}>
        {(d) => (
          <>
            <p class="commit-detail-sha">{d().sha}</p>
            <Show when={d().decorations.length > 0}>
              <div class="commit-detail-decorations">
                <For each={d().decorations}>{(dec) => <span class="badge">{dec}</span>}</For>
              </div>
            </Show>
            <p class="commit-detail-subject">{d().subject}</p>
            <Show when={d().body}>
              <pre class="commit-detail-body">{d().body}</pre>
            </Show>
            <dl class="commit-detail-meta">
              <dt>Author</dt>
              <dd>
                {d().author_name} &lt;{d().author_email}&gt; — {d().author_date}
              </dd>
              <dt>Committer</dt>
              <dd>
                {d().committer_name} &lt;{d().committer_email}&gt; — {d().committer_date}
              </dd>
              <dt>Parents</dt>
              <dd>
                <Show when={d().parents.length > 0} fallback="none (root commit)">
                  <For each={d().parents}>{(p) => <span class="commit-detail-parent">{p.slice(0, 7)}</span>}</For>
                </Show>
              </dd>
              <dt>Signature</dt>
              <dd class={signatureLabel(d().signature).class}>{signatureLabel(d().signature).text}</dd>
            </dl>
            <Show when={d().parents.length > 1}>
              <div class="commit-detail-diff-basis">
                <span>Diffed against:</span>
                <select
                  value={d().diff_parent_index ?? 0}
                  onChange={(e) => setParentIndex(Number(e.currentTarget.value))}
                >
                  <For each={d().parents}>
                    {(p, i) => (
                      <option value={i()}>
                        parent {i() + 1} ({p.slice(0, 7)})
                      </option>
                    )}
                  </For>
                </select>
              </div>
            </Show>
            <table class="commit-detail-files">
              <For each={d().files}>
                {(f) => (
                  <>
                    <tr>
                      <td class="commit-detail-file-path">{f.path}</td>
                      <td class="commit-detail-file-add">{f.additions === null ? "binary" : `+${f.additions}`}</td>
                      <td class="commit-detail-file-del">{f.deletions === null ? "" : `-${f.deletions}`}</td>
                      <td class="commit-detail-file-actions">
                        <Show when={props.onSelectFileHistory}>
                          <button
                            class="working-copy-action"
                            onClick={() => props.onSelectFileHistory!(f.path)}
                            title="View history for this file"
                          >
                            History
                          </button>
                        </Show>
                        <Show when={props.onOpenBlame}>
                          <button
                            class="working-copy-action"
                            onClick={() => props.onOpenBlame!(f.path, props.sha)}
                            title="View blame for this file at this commit"
                          >
                            Blame
                          </button>
                        </Show>
                        <Show when={props.onViewRevision}>
                          <button
                            class="working-copy-action"
                            onClick={() => props.onViewRevision!(f.path, props.sha)}
                            title="Open historical revision read-only"
                          >
                            View
                          </button>
                        </Show>
                      </td>
                    </tr>
                  </>
                )}
              </For>
            </table>
          </>
        )}
      </Show>
    </div>
  );
};
