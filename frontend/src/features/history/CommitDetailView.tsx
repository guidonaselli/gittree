import { type Component, For, Show, createResource } from "solid-js";
import { getCommitDetail, type SignatureState } from "../../api/commands";

function signatureLabel(state: SignatureState): { text: string; class: string } {
  if (state === "Unsigned") return { text: "Not signed", class: "signature-unsigned" };
  if ("Valid" in state) return { text: `Valid signature (${state.Valid.signer})`, class: "signature-valid" };
  if ("Invalid" in state) return { text: `Invalid signature: ${state.Invalid.reason}`, class: "signature-invalid" };
  return { text: `Signature unverifiable: ${state.Unverifiable.reason}`, class: "signature-unverifiable" };
}

export const CommitDetailView: Component<{ root: string; sha: string }> = (props) => {
  const [detail] = createResource(
    () => [props.root, props.sha] as const,
    ([root, sha]) => getCommitDetail(root, sha),
  );

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
            <table class="commit-detail-files">
              <For each={d().files}>
                {(f) => (
                  <tr>
                    <td class="commit-detail-file-path">{f.path}</td>
                    <td class="commit-detail-file-add">{f.additions === null ? "binary" : `+${f.additions}`}</td>
                    <td class="commit-detail-file-del">{f.deletions === null ? "" : `-${f.deletions}`}</td>
                  </tr>
                )}
              </For>
            </table>
          </>
        )}
      </Show>
    </div>
  );
};
