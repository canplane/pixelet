// @pxl/web/platform/web/backdrop.ts

import type { ViewStyle } from "react-native";

import { alpha, type Material } from "@/shared/ui/theme.ts";

export function backdrop(fill: string, m: Material): ViewStyle {
  // RN's style type has no backdrop filter; react-native-web writes it out.
  return { backgroundColor: alpha(fill, m.opacity), backdropFilter: `blur(${m.blur}px)` } as ViewStyle;
}
