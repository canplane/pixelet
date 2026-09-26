// @pxl/web/platform/web/above-keyboard.tsx

import type { ReactNode } from "react";

interface AboveKeyboardProps {
  /** From the screen's side and bottom edges, and from the keyboard's top. */
  inset: number;
  /** Further up than `inset`, over what stands below it. */
  lift?: number;
  children: ReactNode;
}

/**
 * A layer across the bottom of the screen that stands on the keyboard while
 * one is open: on the web, over what the visual viewport has lost to it
 * (`--vv-bottom`, visual-viewport.ts). A CSS variable rather than a value
 * the layer re-renders on, since it follows the keyboard every frame. A
 * device's keyboard reports its own frame, and its build puts that here.
 *
 * iOS delivers visualViewport events inconsistently (streamed while the
 * keyboard opens, or as one final value if it was already up), so `bottom`
 * gets a short ease — long enough to read as motion, short enough to stay
 * attached during the streaming case.
 */
export function AboveKeyboard({ inset, lift = 0, children }: AboveKeyboardProps) {
  return (
    <div
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: `calc(${inset + lift}px + var(--vv-bottom, 0px))`,
        zIndex: 40,
        transition: "bottom 150ms ease-out",
        pointerEvents: "none",
      }}
    >
      {children}
    </div>
  );
}
