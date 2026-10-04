// @pxl/web/shared/ui/bottom-rest.ts

import type { ReactNode } from "react";

export interface SafeArea {
  top: number;
  bottom: number;
  /** The part of `bottom` that takes a touch: Android's navigation buttons, never a gesture area. */
  tappable: number;
}

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
