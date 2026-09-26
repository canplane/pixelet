// @pxl/web/platform/web/glass.tsx

/**
 * A floating control's ground, on the web: its fill. A device draws the
 * system's glass in this place where it has one (Liquid Glass, from iOS 26);
 * a page has no system glass, so the control stays the solid it was designed
 * as, and its presence (shared/ui/motion/veil.ts) is its opacity.
 */

import type { ReactNode } from "react";
import type { ColorValue, StyleProp, ViewStyle } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Motion } from "@platform/motion.tsx";
import { SNAP, type Transition } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";

export interface GlassProps {
  /** The ground where there is no glass. */
  fill: ColorValue;
  /** The glass's tint where there is glass. */
  tint?: ColorValue;
  /** Whether the glass answers a touch, as a control's does. */
  interactive?: boolean;
  /** How present it is, from 1 to 0 (gone); and what it mounts at. */
  fade?: f64;
  fadeFrom?: f64;
  /** How a change of `fade` is moved through. */
  transition?: Transition;
  /** Its box, border and shadow; no transform, which is its fade's. */
  style?: StyleProp<ViewStyle>;
  children?: ReactNode;
}

/** Whether a surface here is the system's glass: a page has none. */
export const SYSTEM_GLASS: boolean = false;

export function Glass({ fill, fade = 1, fadeFrom, transition = SNAP, style, children }: GlassProps) {
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
