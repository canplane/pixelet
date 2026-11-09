// @pxl/client/ui/shared/component/backdrop.native.ts

import type { ViewStyle } from "react-native";

import type { Material } from "@/ui/shared/component/theme.ts";

/** A device cannot blur the world, which it draws in a native surface, and a
 * see-through fill without a blur is unreadable, so a material's fill is
 * solid. */
export function backdrop(fill: string, _m: Material): ViewStyle {
  return { backgroundColor: fill };
}
