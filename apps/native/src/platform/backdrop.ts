// @pxl/native/platform/backdrop.ts

import type { ViewStyle } from "react-native";

import type { Material } from "@/shared/ui/theme.ts";

/**
 * A see-through backing where nothing blurs what is behind it (the page's is
 * platform/web/backdrop.ts): its fill, solid. Android cannot blur: the map is
 * drawn in a SurfaceView, a window of its own that a view's blur cannot read,
 * and React Native has no backdrop blur over its own views; and a fill left
 * see-through without one shows what is behind it sharp, a mess under a
 * header's words or a flyout's marks. An iPhone blurs for itself
 * (backing.tsx) and asks nothing of this.
 */
export function backdrop(fill: string, _m: Material): ViewStyle {
  return { backgroundColor: fill };
}
