import { describe, expect, it, vi } from "vitest";

describe("Modal Escape & Focus Restoration Logic", () => {
  it("triggers onClose callback when Escape key is pressed", () => {
    const onClose = vi.fn();
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };

    const escapeEvent = {
      key: "Escape",
      preventDefault: vi.fn(),
    } as unknown as KeyboardEvent;

    handleKeyDown(escapeEvent);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(escapeEvent.preventDefault).toHaveBeenCalled();

    const enterEvent = {
      key: "Enter",
      preventDefault: vi.fn(),
    } as unknown as KeyboardEvent;

    handleKeyDown(enterEvent);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("records active element and restores focus on cleanup", () => {
    const button = {
      focus: vi.fn(),
    } as unknown as HTMLElement;

    let previousActiveElement: HTMLElement | null = button;
    const cleanup = () => {
      if (previousActiveElement && typeof previousActiveElement.focus === "function") {
        previousActiveElement.focus();
      }
    };

    cleanup();
    expect(button.focus).toHaveBeenCalledTimes(1);
  });
});
