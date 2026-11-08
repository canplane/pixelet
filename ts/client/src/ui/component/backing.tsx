// @pxl/client/ui/component/backing.tsx

/** A page and Android have no system glass, so the backing is a plain fill
 * that fades in and out by its opacity. */

import type { ReactNode } from "react";
import type { ColorValue, StyleProp, ViewStyle } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Motion } from "@/ui/component/motion/motion.tsx";
import { SNAP, type Transition } from "@/ui/component/motion/transition.ts";
import { useDim } from "@/ui/component/motion/dim.ts";
import type { Material } from "@/ui/component/theme.ts";
import { backdrop } from "./backdrop.ts";
import * as BackingPlace from "./backing-place.ts";

type BackingPlace = (typeof BackingPlace)[keyof typeof BackingPlace];
export { BackingPlace };

export interface BackingProps {
  /** With a material, the fill must be a `#rrggbb` color, and the material
   * sets its opacity. */
  fill: ColorValue;
  material?: Material;
  tint?: ColorValue;
  interactive?: boolean;
  place: BackingPlace;
  fade?: f64;
  fadeFrom?: f64;
  transition?: Transition;
  /** The style must not set a transform: the Motion that fades the backing
   * writes its own transform over it. */
  style?: StyleProp<ViewStyle>;
  children?: ReactNode;
}

export const SYSTEM_BACKING: boolean = false;

export function Backing({
  fill,
  material,
  fade = 1,
  fadeFrom,
  transition = SNAP,
  style,
  children,
}: BackingProps) {
  const dim = useDim(fade, transition, fadeFrom);
  return (
    <Motion
      {...dim.motion}
      style={[
        style,
        material
          ? backdrop(fill as string, material)
          : { backgroundColor: fill },
      ]}
    >
      {children}
    </Motion>
  );
}
