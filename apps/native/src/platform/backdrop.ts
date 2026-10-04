// @pxl/native/platform/backdrop.ts

import type { ViewStyle } from "react-native";

import type { Material } from "@/shared/ui/theme.ts";

/**
 * Solid fill: Android cannot blur the map, which is a SurfaceView, and an
 * unblurred see-through fill is unreadable. An iPhone blurs in backing.tsx.
 */
export function backdrop(fill: string, _m: Material): ViewStyle {
  return { backgroundColor: fill };
}
