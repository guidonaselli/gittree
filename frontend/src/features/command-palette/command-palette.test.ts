import { describe, expect, it, vi } from "vitest";
import type { CommandPaletteItem } from "./CommandPalette";

describe("Command Palette Logic", () => {
  const sampleItems: CommandPaletteItem[] = [
    {
      id: "fetch",
      title: "Fetch from All Remotes",
      category: "Sync",
      shortcut: "Ctrl+Shift+F",
      available: true,
      onExecute: vi.fn(),
    },
    {
      id: "push-force",
      title: "Force Push",
      category: "Sync",
      available: true,
      isDestructive: true,
      onExecute: vi.fn(),
    },
    {
      id: "stage-all",
      title: "Stage All Changes",
      category: "Working Copy",
      shortcut: "Ctrl+Alt+S",
      available: false,
      unavailableReason: "No unstaged changes in working copy",
      onExecute: vi.fn(),
    },
    {
      id: "switch-repo-1",
      title: "Switch to repo: gittree",
      category: "Repositories",
      available: true,
      onExecute: vi.fn(),
    },
  ];

  it("filters items by title case-insensitively", () => {
    const q = "fetch";
    const filtered = sampleItems.filter((item) =>
      item.title.toLowerCase().includes(q) ||
      item.category.toLowerCase().includes(q) ||
      (item.shortcut && item.shortcut.toLowerCase().includes(q))
    );
    expect(filtered.length).toBe(1);
    expect(filtered[0].id).toBe("fetch");
  });

  it("filters items by category", () => {
    const q = "sync";
    const filtered = sampleItems.filter((item) =>
      item.title.toLowerCase().includes(q) ||
      item.category.toLowerCase().includes(q) ||
      (item.shortcut && item.shortcut.toLowerCase().includes(q))
    );
    expect(filtered.length).toBe(2);
    expect(filtered.map((i) => i.id)).toEqual(["fetch", "push-force"]);
  });

  it("filters items by shortcut", () => {
    const q = "shift+f";
    const filtered = sampleItems.filter((item) =>
      item.title.toLowerCase().includes(q) ||
      item.category.toLowerCase().includes(q) ||
      (item.shortcut && item.shortcut.toLowerCase().includes(q))
    );
    expect(filtered.length).toBe(1);
    expect(filtered[0].id).toBe("fetch");
  });

  it("preserves unavailable reason for disabled commands", () => {
    const disabled = sampleItems.find((i) => !i.available);
    expect(disabled).toBeDefined();
    expect(disabled?.unavailableReason).toBe("No unstaged changes in working copy");
  });

  it("marks destructive actions distinctly", () => {
    const destructive = sampleItems.filter((i) => i.isDestructive);
    expect(destructive.length).toBe(1);
    expect(destructive[0].id).toBe("push-force");
  });

  it("executes action only if available", () => {
    const execute = (item: CommandPaletteItem) => {
      if (!item.available) return false;
      item.onExecute();
      return true;
    };

    const availableItem = sampleItems[0];
    const unavailableItem = sampleItems[2];

    expect(execute(availableItem)).toBe(true);
    expect(availableItem.onExecute).toHaveBeenCalled();

    expect(execute(unavailableItem)).toBe(false);
    expect(unavailableItem.onExecute).not.toHaveBeenCalled();
  });
});
