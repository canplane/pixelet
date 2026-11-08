// @pxl/client/ui/component/backdrop.native.ts

import type { ViewStyle } from "react-native";

import type { Material } from "@/ui/component/theme.ts";

/** A material's fill on a device: solid. A device cannot blur the world,
 * which it draws in a native surface, and a see-through fill without a blur
 * is unreadable. */
export function backdrop(fill: string, _m: Material): ViewStyle {
  return { backgroundColor: fill };
}
