// @pxl/client/ui/component/bottom-rest.ts

import type { ReactNode } from "react";

import type { SafeArea } from "@/service/types.ts";

export interface AboveKeyboardProps {
  inset: number;
  lift?: number;
  children: ReactNode;
}

/**
 * How far above the screen's bottom the chrome rests when no keyboard is up.
 * It stands `inset` above Android's tappable navigation buttons. Over an area
 * kept only for gestures, such as the home indicator, it stands `inset` above
 * the bottom, or on top of the area where the area is taller than `inset`.
 */
export function bottomRest(area: SafeArea, inset: number): number {
  return area.tappable + Math.max(inset, area.bottom - area.tappable);
}
