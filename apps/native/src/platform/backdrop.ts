// @pxl/native/platform/backdrop.ts

import type { ViewStyle } from "react-native";

import { alpha, type Material } from "@/shared/ui/theme.ts";

/**
 * A see-through backing on a device that draws it as a fill (the page's is
 * platform/web/backdrop.ts): the fill alone, at the material's bare opacity,
 * since nothing here blurs what is behind it. An iPhone blurs for itself,
 * with its glass or its blur (backing.tsx), and asks nothing of this.
 * Android cannot: the map is drawn
 * in a SurfaceView, a window of its own that a view's blur cannot read, and
 * React Native has no backdrop blur over its own views.
 */
export function backdrop(fill: string, m: Material): ViewStyle {
  return { backgroundColor: alpha(fill, m.bare) };
}
