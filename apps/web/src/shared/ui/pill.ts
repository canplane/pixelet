// @pxl/web/shared/ui/pill.ts

/**
 * A pill whose place and width move, drawn per platform: an iPhone relays it
 * out and the system animates, a page uses transforms, since a width tween
 * ghosts over the canvas on iOS WebKit.
 */

import type { ReactNode } from "react";
import type { ColorValue, GestureResponderHandlers } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import type { Transition } from "@/shared/ui/motion/transition.ts";
import type { BackingPlace } from "@/shared/ui/backing.ts";
import { chrome } from "@/shared/ui/theme.ts";

/** A box around it, not `hitSlop`, which react-native-web does not honor. */
export function touchReach(height: f64): f64 {
  return Math.max(0, (chrome.target - height) / 2);
}

export interface PillProps {
  x: f64;
  width: f64;
  height: f64;
  span: f64;
  fill: ColorValue;
  tint?: ColorValue;
  place: BackingPlace;
  interactive?: boolean;
  border?: { width: f64; color: ColorValue };
  shadow?: string;
  fade?: f64;
  fadeFrom?: f64;
  fadeTransition?: Transition;
  touch?: GestureResponderHandlers;
  /** More of `touch`'s reach under it, over a deck's cards peeking out. */
  touchBelow?: f64;
  children?: ReactNode;
  onContentWidth?: (width: f64) => void;
}
