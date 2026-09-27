// @pxl/web/platform/web/backing.tsx

/**
 * What a floating control stands on — its backing — on the web: its fill.
 * An iPhone draws the system's glass here (Liquid Glass, from iOS 26); a page
 * has none, so the control stays the solid it was designed as, and its
 * presence (shared/ui/motion/veil.ts) is its opacity. Android draws this one.
 */

import type { ReactNode } from "react";
import type { ColorValue, StyleProp, ViewStyle } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Motion } from "@platform/motion.tsx";
import { SNAP, type Transition } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import type { BackingPlace } from "@/shared/ui/backing-place.ts";

export interface BackingProps {
  /** The backing where the system draws none. */
  fill: ColorValue;
  /** The tint of a backing the system draws. */
  tint?: ColorValue;
  /** Whether a backing the system draws answers a touch, as a control's does. */
  interactive?: boolean;
  /** Where it stands (shared/ui/backing-place.ts), which a system's backing
   * is drawn by and a page's is not. */
  place: BackingPlace;
  /** How present it is, from 1 to 0 (gone); and what it mounts at. */
  fade?: f64;
  fadeFrom?: f64;
  /** How a change of `fade` is moved through. */
  transition?: Transition;
  /** Its box, border and shadow; no transform, which is its fade's. */
  style?: StyleProp<ViewStyle>;
  children?: ReactNode;
}

/** Whether the backing here is the system's: a page's never is. */
export const SYSTEM_BACKING: boolean = false;

export function Backing({ fill, fade = 1, fadeFrom, transition = SNAP, style, children }: BackingProps) {
  const veil = useVeil(fade, transition);
  return (
    <Motion
      from={fadeFrom === undefined ? undefined : { opacity: fadeFrom }}
      to={{ opacity: veil.level }}
      transition={veil.transition}
      style={[style, { backgroundColor: fill }]}
    >
      {children}
    </Motion>
  );
}
