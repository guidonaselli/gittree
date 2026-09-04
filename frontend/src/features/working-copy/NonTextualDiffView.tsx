import { type Component, Show, createResource } from "solid-js";
import { getBlobBase64, getWorkingTreeFileBase64 } from "../../api/commands";
import type { NonTextualDiff } from "../../api/types";

const IMAGE_MIME: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  bmp: "image/bmp",
  svg: "image/svg+xml",
  ico: "image/x-icon",
};

function imageMime(path: string): string | null {
  const ext = path.split(".").pop()?.toLowerCase();
  return ext ? (IMAGE_MIME[ext] ?? null) : null;
}

function formatBytes(n: number | null): string {
  return n === null ? "unknown size" : `${n.toLocaleString()} bytes`;
}

function shortSha(sha: string | null): string {
  return sha ? sha.slice(0, 7) : "none";
}

function ImageSide(props: {
  root: string;
  path: string;
  sha: string | null;
  fromWorkingTree: boolean;
  mime: string;
  label: string;
}) {
  const [data] = createResource(
    () => (props.fromWorkingTree ? "working-tree" : props.sha),
    () => (props.fromWorkingTree ? getWorkingTreeFileBase64(props.root, props.path) : getBlobBase64(props.root, props.sha!)),
  );
  const hasSource = () => props.fromWorkingTree || props.sha !== null;
  return (
    <div class="non-textual-image-side">
      <p class="text-muted">{props.label}</p>
      <Show when={hasSource()} fallback={<p class="text-muted">(none)</p>}>
        <Show when={data()}>{(b64) => <img src={`data:${props.mime};base64,${b64()}`} alt={props.label} />}</Show>
      </Show>
    </div>
  );
}

export const NonTextualDiffView: Component<{ root: string; path: string; staged: boolean; kind: NonTextualDiff }> = (
  props,
) => {
  return (
    <div class="non-textual-diff">
      <Show when={"Binary" in props.kind && props.kind}>
        {(k) => {
          const mime = imageMime(props.path);
          return (
            <>
              <p class="non-textual-summary">
                {k().Binary.old_sha === null
                  ? `Binary file added (${formatBytes(k().Binary.new_size)})`
                  : k().Binary.new_sha === null
                    ? `Binary file removed (${formatBytes(k().Binary.old_size)})`
                    : `Binary file changed (${formatBytes(k().Binary.old_size)} → ${formatBytes(k().Binary.new_size)})`}
              </p>
              <Show when={mime}>
                {(m) => (
                  <div class="non-textual-image-pair">
                    <ImageSide root={props.root} path={props.path} sha={k().Binary.old_sha} fromWorkingTree={false} mime={m()} label="Before" />
                    <ImageSide
                      root={props.root}
                      path={props.path}
                      sha={k().Binary.new_sha}
                      fromWorkingTree={!props.staged && k().Binary.new_sha !== null}
                      mime={m()}
                      label="After"
                    />
                  </div>
                )}
              </Show>
            </>
          );
        }}
      </Show>
      <Show when={"Submodule" in props.kind && props.kind}>
        {(k) => (
          <p class="non-textual-summary">
            Submodule updated: {shortSha(k().Submodule.old_commit)} → {shortSha(k().Submodule.new_commit)}
          </p>
        )}
      </Show>
      <Show when={"Symlink" in props.kind && props.kind}>
        {(k) => (
          <p class="non-textual-summary">
            Symlink target changed: {k().Symlink.old_target ?? "(none)"} → {k().Symlink.new_target ?? "(none)"}
          </p>
        )}
      </Show>
      <Show when={"ModeOnly" in props.kind && props.kind}>
        {(k) => (
          <p class="non-textual-summary">
            File mode changed: {k().ModeOnly.old_mode} → {k().ModeOnly.new_mode}
            {k().ModeOnly.new_mode.endsWith("755") && !k().ModeOnly.old_mode.endsWith("755") && " (became executable)"}
            {k().ModeOnly.old_mode.endsWith("755") && !k().ModeOnly.new_mode.endsWith("755") && " (no longer executable)"}
          </p>
        )}
      </Show>
    </div>
  );
};
