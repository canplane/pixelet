// @pxl/web/shared/ui/backing.ts

import type { ReactNode } from "react";
import type { ColorValue, StyleProp, ViewStyle } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Transition } from "@/shared/ui/motion/transition.ts";
import type { Material } from "@/shared/ui/theme.ts";

/** Decides the iPhone's glass style: clear `OnSheet`, regular (frosted)
 * where what is behind moves. A page draws the fill everywhere. */
export const enum BackingPlace {
  OnSheet = 0,
  OverMap = 1,
  OverContent = 2,
}

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
