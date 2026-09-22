/**
 * Keybindings system with conflict detection and event matching.
 */

export interface KeybindingAction {
  id: string;
  title: string;
  category: "General" | "Navigation" | "Working Copy" | "Sync" | "History";
  defaultShortcut: string;
  description?: string;
  isDestructive?: boolean;
}

export const KEYBINDING_ACTIONS: KeybindingAction[] = [
  {
    id: "command-palette",
    title: "Open Command Palette",
    category: "General",
    defaultShortcut: "Ctrl+K",
    description: "Search and execute any command or switch repositories",
  },
  {
    id: "view-working-copy",
    title: "Go to Working Copy",
    category: "Navigation",
    defaultShortcut: "Ctrl+1",
  },
  {
    id: "view-history",
    title: "Go to History",
    category: "Navigation",
    defaultShortcut: "Ctrl+2",
  },
  {
    id: "view-branches",
    title: "Go to Branches",
    category: "Navigation",
    defaultShortcut: "Ctrl+3",
  },
  {
    id: "view-tags",
    title: "Go to Tags",
    category: "Navigation",
    defaultShortcut: "Ctrl+4",
  },
  {
    id: "view-stashes",
    title: "Go to Stashes",
    category: "Navigation",
    defaultShortcut: "Ctrl+5",
  },
  {
    id: "view-reflog",
    title: "Go to Reflog",
    category: "Navigation",
    defaultShortcut: "Ctrl+6",
  },
  {
    id: "next-tab",
    title: "Next Tab",
    category: "Navigation",
    defaultShortcut: "Ctrl+Tab",
  },
  {
    id: "prev-tab",
    title: "Previous Tab",
    category: "Navigation",
    defaultShortcut: "Ctrl+Shift+Tab",
  },
  {
    id: "commit",
    title: "Commit Working Copy",
    category: "Working Copy",
    defaultShortcut: "Ctrl+Enter",
  },
  {
    id: "stage-all",
    title: "Stage All Tracked & Untracked",
    category: "Working Copy",
    defaultShortcut: "Ctrl+Shift+A",
  },
  {
    id: "discard-all",
    title: "Discard Working Copy Changes",
    category: "Working Copy",
    defaultShortcut: "Ctrl+Shift+D",
    isDestructive: true,
  },
  {
    id: "fetch",
    title: "Fetch Remotes",
    category: "Sync",
    defaultShortcut: "Ctrl+Shift+F",
  },
  {
    id: "pull",
    title: "Pull from Upstream",
    category: "Sync",
    defaultShortcut: "Ctrl+Shift+L",
  },
  {
    id: "push",
    title: "Push to Remote",
    category: "Sync",
    defaultShortcut: "Ctrl+Shift+P",
  },
  {
    id: "refresh-state",
    title: "Refresh Repository State",
    category: "General",
    defaultShortcut: "Ctrl+R",
  },
];

export function normalizeShortcut(shortcut: string): string {
  const parts = shortcut
    .trim()
    .split("+")
    .map((p) => p.trim());
  const modifiers: string[] = [];
  let key = "";

  for (const p of parts) {
    const lower = p.toLowerCase();
    if (lower === "ctrl" || lower === "control") modifiers.push("Ctrl");
    else if (lower === "alt" || lower === "opt" || lower === "option") modifiers.push("Alt");
    else if (lower === "shift") modifiers.push("Shift");
    else if (lower === "meta" || lower === "cmd" || lower === "command") modifiers.push("Meta");
    else key = p.length === 1 ? p.toUpperCase() : p;
  }

  // Stable sort modifiers: Ctrl, Alt, Shift, Meta
  const order = ["Ctrl", "Alt", "Shift", "Meta"];
  modifiers.sort((a, b) => order.indexOf(a) - order.indexOf(b));

  return [...modifiers, key].filter(Boolean).join("+");
}

export function matchesEvent(shortcut: string, event: KeyboardEvent): boolean {
  if (!shortcut) return false;
  const parts = shortcut.split("+").map((p) => p.trim().toLowerCase());
  const needsCtrl = parts.includes("ctrl") || parts.includes("control");
  const needsAlt = parts.includes("alt") || parts.includes("option");
  const needsShift = parts.includes("shift");
  const needsMeta = parts.includes("meta") || parts.includes("cmd");

  if (event.ctrlKey !== needsCtrl) return false;
  if (event.altKey !== needsAlt) return false;
  if (event.shiftKey !== needsShift) return false;
  if (event.metaKey !== needsMeta) return false;

  const keyPart = parts.find(
    (p) => !["ctrl", "control", "alt", "option", "shift", "meta", "cmd"].includes(p)
  );
  if (!keyPart) return false;

  const eventKey = event.key.toLowerCase();
  if (keyPart === "tab") return eventKey === "tab";
  if (keyPart === "enter") return eventKey === "enter";
  if (keyPart === "escape" || keyPart === "esc") return eventKey === "escape";

  return eventKey === keyPart;
}

export interface RebindOutcome {
  success: boolean;
  conflictingAction?: KeybindingAction;
}

export function validateRebind(
  actionId: string,
  newShortcut: string,
  customBindings: Record<string, string>
): RebindOutcome {
  const normalized = normalizeShortcut(newShortcut);
  if (!normalized) return { success: false };

  // Check against all actions (custom or default)
  for (const action of KEYBINDING_ACTIONS) {
    if (action.id === actionId) continue;
    const currentShortcut = normalizeShortcut(
      customBindings[action.id] ?? action.defaultShortcut
    );
    if (currentShortcut === normalized) {
      return {
        success: false,
        conflictingAction: action,
      };
    }
  }

  return { success: true };
}

const STORAGE_KEY = "gittree:keybindings";

export function loadCustomBindings(): Record<string, string> {
  if (typeof localStorage === "undefined") return {};
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

export function saveCustomBindings(bindings: Record<string, string>): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(bindings));
  } catch {
    // LocalStorage failure tolerated
  }
}

export function getEffectiveShortcut(
  actionId: string,
  customBindings: Record<string, string>
): string {
  if (customBindings[actionId]) return customBindings[actionId];
  const def = KEYBINDING_ACTIONS.find((a) => a.id === actionId);
  return def ? def.defaultShortcut : "";
}
