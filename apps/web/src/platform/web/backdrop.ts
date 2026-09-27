// @pxl/web/platform/web/backdrop.ts

import type { ViewStyle } from "react-native";

import { alpha, type Material } from "@/shared/ui/theme.ts";

/**
 * A see-through backing on a page (shared/ui/theme.ts `Material`): its fill
 * at the material's opacity, over what is behind it blurred (CSS
 * `backdrop-filter`), so it reads over any ground as a device's glass does.
 * A device's is beside the device app's (platform/backdrop.ts).
 */
export function backdrop(fill: string, m: Material): ViewStyle {
  // React Native's style type has no backdrop filter; react-native-web
  // writes it out, prefixed where a browser needs it.
  return { backgroundColor: alpha(fill, m.opacity), backdropFilter: `blur(${m.blur}px)` } as ViewStyle;
}
