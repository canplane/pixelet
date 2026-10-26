// @pxl/client/shell/ui/backing.tsx

/**
 * A floating control's backing: its fill, on a page and on Android. A page
 * has no system glass, so presence is plain opacity. An iPhone's is glass
 * (backing.ios.tsx).
 */

import type { ReactNode } from "react";
import {
  type ColorValue,
  Platform,
  type StyleProp,
  type ViewStyle,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SNAP, type Transition } from "@/shell/ui/motion/transition.ts";
import { useVeil } from "@/shell/ui/motion/veil.ts";
import { alpha, type Material } from "@/shell/ui/theme.ts";

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

export const SYSTEM_BACKING: boolean = false;

/**
 * A see-through fill blurs what is behind on a page. Android cannot blur the
 * map, which is a SurfaceView, and an unblurred see-through fill is
 * unreadable, so it takes the fill solid.
 */
function backdrop(fill: string, m: Material): ViewStyle {
  if (Platform.OS !== "web") return { backgroundColor: fill };
  // RN's style type has no backdrop filter; react-native-web writes it out.
  return {
    backgroundColor: alpha(fill, m.opacity),
    backdropFilter: `blur(${m.blur}px)`,
  } as ViewStyle;
}

export function Backing(
  { fill, material, fade = 1, fadeFrom, transition = SNAP, style, children }:
    BackingProps,
) {
  const veil = useVeil(fade, transition, fadeFrom);
  return (
    <Motion
      {...veil.motion}
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
