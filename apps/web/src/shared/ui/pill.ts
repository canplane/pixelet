// @pxl/web/shared/ui/pill.ts

/**
 * A pill that moves and changes width as a control does — a circle opening
 * into a pill and back — drawn by each platform its own way
 * (platform/web/pill.tsx and the device app's platform/pill.tsx): a device lays the one pill out anew and the system
 * animates its frame, so the platform's glass is redrawn whole at every
 * width; a page moves it by transforms alone, since a width tween over the
 * canvas ghosts on iOS's WebKit. Its caller says where it stands and how wide
 * it shows, and never how it gets there.
 */

import type { ReactNode } from "react";
import type { ColorValue, GestureResponderHandlers } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import type { Transition } from "@/shared/ui/motion/transition.ts";
import type { GlassVariant } from "@/shared/ui/glass-variant.ts";

export interface PillProps {
  /** Where its left end stands, in its container. */
  x: f64;
  /** The width it shows from its left end; at its height, a circle. */
  width: f64;
  height: f64;
  /** The widest it is ever asked to show. */
  span: f64;
  fill: ColorValue;
  /** The glass's tint, where there is glass. */
  tint?: ColorValue;
  /** The glass thickness (shared/ui/glass-variant.ts). */
  variant: GlassVariant;
  /** Whether the glass answers a touch, where there is glass: a pill that is
   * one button, not one that holds several, whose own buttons answer. */
  interactive?: boolean;
  border?: { width: f64; color: ColorValue };
  shadow?: string;
  /** How present it is, from 1 to 0 (gone), what it mounts at, and how a
   * change is moved through (shared/ui/motion/veil.ts). */
  fade?: f64;
  fadeFrom?: f64;
  fadeTransition?: Transition;
  /** What a finger on it does. */
  touch?: GestureResponderHandlers;
  /** Laid out from its left end at its own width, and shown as far as the
   * pill reaches. */
  children?: ReactNode;
  /** The content's own width, whenever it is laid out. */
  onContentWidth?: (width: f64) => void;
}
