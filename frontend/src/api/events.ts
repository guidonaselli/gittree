import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// Backend-emitted filesystem watcher events.
export function onRepositoryChanged(handler: (root: string) => void): Promise<UnlistenFn> {
  return listen<string>("repo:changed", (event) => handler(event.payload));
}

export function onWatchDegraded(handler: (info: { root: string; reason: string }) => void): Promise<UnlistenFn> {
  return listen<{ root: string; reason: string }>("repo:watch-degraded", (event) => handler(event.payload));
}

export function onDesktopThemeChanged(handler: () => void): Promise<UnlistenFn> {
  return listen<void>("desktop-theme:changed", () => handler());
}

export function onAskpassPrompt(
  handler: (payload: import("./types").AskpassPromptPayload) => void
): Promise<UnlistenFn> {
  return listen<import("./types").AskpassPromptPayload>("askpass:prompt", (event) =>
    handler(event.payload)
  );
}
