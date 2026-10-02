// @pxl/web/platform/web/above-keyboard.tsx

import type { ReactNode } from "react";

import { useSafeArea } from "@platform/safe-area.ts";
import { bottomRest } from "@/shared/ui/bottom-rest.ts";

interface AboveKeyboardProps {
  inset: number;
  lift?: number;
  children: ReactNode;
}

/**
 * A bottom layer standing `inset` above the keyboard via `--vv-bottom`, a CSS
 * variable since it follows every frame. iOS streams viewport events or sends
 * one final value, so `bottom` gets a short ease.
 */
export function AboveKeyboard({ inset, lift = 0, children }: AboveKeyboardProps) {
  const rest = bottomRest(useSafeArea(), inset);
  return (
    <div
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: `calc(max(${rest}px, ${inset}px + var(--vv-bottom, 0px)) + ${lift}px)`,
        zIndex: 40,
        transition: "bottom 150ms ease-out",
        pointerEvents: "none",
      }}
    >
      {children}
    </div>
  );
}
