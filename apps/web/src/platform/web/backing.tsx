// @pxl/web/platform/web/backing.tsx

/**
 * A floating control's backing on the web (and Android): its fill. A page
 * has no system glass, so presence is plain opacity.
 */

import type { ReactNode } from "react";
import type { ColorValue, StyleProp, ViewStyle } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Motion } from "@platform/motion.tsx";
import { SNAP, type Transition } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import type { BackingPlace } from "@/shared/ui/backing-place.ts";
import { backdrop } from "@platform/backdrop.ts";
import type { Material } from "@/shared/ui/theme.ts";

export interface BackingProps {
  /** With a material, a `#rrggbb` the material sets the opacity of. */
  fill: ColorValue;
  material?: Material;
  tint?: ColorValue;
  interactive?: boolean;
  place: BackingPlace;
  fade?: f64;
  fadeFrom?: f64;
  transition?: Transition;
  /** No transform: that is the fade's. */
  style?: StyleProp<ViewStyle>;
  children?: ReactNode;
}

export const SYSTEM_BACKING: boolean = false;

export function Backing({ fill, material, fade = 1, fadeFrom, transition = SNAP, style, children }: BackingProps) {
  const veil = useVeil(fade, transition);
  return (
    <Motion
      from={fadeFrom === undefined ? undefined : { opacity: fadeFrom }}
      to={{ opacity: veil.level }}
      transition={veil.transition}
      style={[style, material ? backdrop(fill as string, material) : { backgroundColor: fill }]}
    >
      {children}
    </Motion>
  );
}
