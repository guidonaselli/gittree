import { describe, expect, it } from "vitest";
import {
  matchesEvent,
  normalizeShortcut,
  validateRebind,
  KEYBINDING_ACTIONS,
  getEffectiveShortcut,
} from "./keybindings";

describe("Keybindings System", () => {
  it("normalizes shortcuts with consistent modifier ordering", () => {
    expect(normalizeShortcut("shift+ctrl+p")).toBe("Ctrl+Shift+P");
    expect(normalizeShortcut("ctrl + k")).toBe("Ctrl+K");
    expect(normalizeShortcut("alt+ctrl+t")).toBe("Ctrl+Alt+T");
    expect(normalizeShortcut("cmd+p")).toBe("Meta+P");
  });

  it("exposes all keybinding actions with id, title and category", () => {
    expect(KEYBINDING_ACTIONS.length).toBeGreaterThan(0);
    for (const action of KEYBINDING_ACTIONS) {
      expect(action.id).toBeDefined();
      expect(action.title).toBeDefined();
      expect(action.category).toBeDefined();
    }
  });

  it("matches KeyboardEvent accurately", () => {
    const ctrlKEvent = {
      key: "k",
      ctrlKey: true,
      altKey: false,
      shiftKey: false,
      metaKey: false,
    } as KeyboardEvent;
    expect(matchesEvent("Ctrl+K", ctrlKEvent)).toBe(true);
    expect(matchesEvent("Ctrl+Shift+K", ctrlKEvent)).toBe(false);
    expect(matchesEvent("Ctrl+P", ctrlKEvent)).toBe(false);

    const ctrlShiftFEvent = {
      key: "F",
      ctrlKey: true,
      altKey: false,
      shiftKey: true,
      metaKey: false,
    } as KeyboardEvent;
    expect(matchesEvent("Ctrl+Shift+F", ctrlShiftFEvent)).toBe(true);
  });

  it("detects conflict when rebinding to a shortcut already used by another action", () => {
    // "Ctrl+K" is used by "command-palette"
    const outcome = validateRebind("view-working-copy", "Ctrl+K", {});
    expect(outcome.success).toBe(false);
    expect(outcome.conflictingAction).toBeDefined();
    expect(outcome.conflictingAction?.id).toBe("command-palette");
    expect(outcome.conflictingAction?.title).toBe("Open Command Palette");
  });

  it("allows rebinding to an unused shortcut", () => {
    const outcome = validateRebind("view-working-copy", "Ctrl+Alt+W", {});
    expect(outcome.success).toBe(true);
    expect(outcome.conflictingAction).toBeUndefined();
  });

  it("takes custom bindings into account when detecting conflicts", () => {
    const custom = {
      "view-history": "Ctrl+H",
    };
    // Rebinding another action to custom "Ctrl+H" should conflict
    const outcome = validateRebind("fetch", "Ctrl+H", custom);
    expect(outcome.success).toBe(false);
    expect(outcome.conflictingAction?.id).toBe("view-history");
  });

  it("returns effective shortcut falling back to default", () => {
    expect(getEffectiveShortcut("command-palette", {})).toBe("Ctrl+K");
    expect(getEffectiveShortcut("command-palette", { "command-palette": "Ctrl+P" })).toBe("Ctrl+P");
  });
});
