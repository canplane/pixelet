// @pxl/client/ui/component/backdrop.ts

import type { ViewStyle } from "react-native";

import { alpha, type Material } from "@/ui/component/theme.ts";

/** A material's fill on a page: see-through, blurring what is behind it.
 * React Native's style type has no backdrop filter, but react-native-web
 * writes it out to the page's CSS. A device draws it solid
 * (backdrop.native.ts). */
export function backdrop(fill: string, m: Material): ViewStyle {
  return {
    backgroundColor: alpha(fill, m.opacity),
    backdropFilter: `blur(${m.blur}px)`,
  } as ViewStyle;
}
