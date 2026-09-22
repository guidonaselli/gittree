import { onCleanup, onMount } from "solid-js";

/**
 * Hook to provide an Escape key path out of any modal and restore
 * focus to the previously active element when the modal unmounts.
 */
export function useModalEscape(onClose: () => void): void {
  let previousActiveElement: HTMLElement | null = null;

  onMount(() => {
    previousActiveElement = document.activeElement as HTMLElement | null;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    onCleanup(() => {
      window.removeEventListener("keydown", handleKeyDown, true);
      if (previousActiveElement && typeof previousActiveElement.focus === "function") {
        previousActiveElement.focus();
      }
    });
  });
}
