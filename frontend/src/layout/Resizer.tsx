import { type Component } from "solid-js";

type Props = {
  onResize: (deltaPx: number) => void;
  orientation?: "vertical";
  label: string;
};

/**
 * A draggable divider between two panes. Keyboard-operable per spec
 * (ui-shell-and-theming, "Keyboard operability"): focusable, and
 * ArrowLeft/ArrowRight resize in 16px steps without a pointer.
 */
export const Resizer: Component<Props> = (props) => {
  let dragging = false;
  let startX = 0;

  const onPointerDown = (e: PointerEvent) => {
    dragging = true;
    startX = e.clientX;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: PointerEvent) => {
    if (!dragging) return;
    const delta = e.clientX - startX;
    startX = e.clientX;
    props.onResize(delta);
  };
  const onPointerUp = () => {
    dragging = false;
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "ArrowLeft") props.onResize(-16);
    if (e.key === "ArrowRight") props.onResize(16);
  };

  return (
    <div
      class="resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label={props.label}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onKeyDown={onKeyDown}
    />
  );
};
