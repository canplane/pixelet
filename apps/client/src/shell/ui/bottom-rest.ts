// @pxl/client/shell/ui/bottom-rest.ts

import type { ReactNode } from "react";

import type { SafeArea } from "@/bracket.tsx";

export interface AboveKeyboardProps {
  inset: number;
  lift?: number;
  children: ReactNode;
}

/**
 * How far above the screen's bottom the chrome rests with no keyboard:
 * `inset` above Android's tappable navigation buttons, but on top of a
 * gesture-only area (home indicator) where that is higher than `inset`.
 */
export function bottomRest(area: SafeArea, inset: number): number {
  return area.tappable + Math.max(inset, area.bottom - area.tappable);
}
