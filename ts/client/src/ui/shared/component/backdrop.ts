// @pxl/client/ui/shared/component/backdrop.ts

import type { ViewStyle } from "react-native";

import { alpha, type Material } from "@/ui/shared/component/theme.ts";

/** React Native's style type has no backdrop filter, but react-native-web
 * writes it out to the page's CSS. */
export function backdrop(fill: string, m: Material): ViewStyle {
  return {
    backgroundColor: alpha(fill, m.opacity),
    backdropFilter: `blur(${m.blur}px)`,
  } as ViewStyle;
}
