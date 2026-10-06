import { render } from "solid-js/web";
import { App } from "./App";
import "./theme/tokens.css";

// Browser preview fallback when running outside Tauri desktop shell
if (typeof window !== "undefined" && !(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) {
  (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown>; transformCallback: (cb: unknown) => unknown } }).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string) => {
      if (cmd === "get_settings") return { settings: { concurrency: 8, theme: null }, error: null };
      if (cmd === "get_bookmarks") return [{ root: "/home/gnaselli/work/gittree", group: "Work" }];
      if (cmd === "get_user_themes") return { themes: [], errors: [] };
      if (cmd === "get_operation_log") return [];
      if (cmd === "pick_folder") return "/home/gnaselli/work/gittree";
      return null;
    },
    transformCallback: (callback: unknown) => callback,
  };
}

const root = document.getElementById("root");
if (!root) {
  throw new Error("#root element is missing from index.html");
}

render(() => <App />, root);
