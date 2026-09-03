import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// Backend-emitted filesystem watcher events.
export function onRepositoryChanged(handler: (root: string) => void): Promise<UnlistenFn> {
  return listen<string>("repo:changed", (event) => handler(event.payload));
}

export function onWatchDegraded(handler: (info: { root: string; reason: string }) => void): Promise<UnlistenFn> {
  return listen<{ root: string; reason: string }>("repo:watch-degraded", (event) => handler(event.payload));
}
