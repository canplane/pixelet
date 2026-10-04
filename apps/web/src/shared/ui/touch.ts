// @pxl/web/shared/ui/touch.ts

import { StyleSheet } from "react-native";

/** Whether a layer takes touches, from a sheet: RN-web draws an inline `box-none` as `none`, which every control under it inherits. */
export const touches = StyleSheet.create({
  pass: { pointerEvents: "box-none" },
  block: { pointerEvents: "none" },
});
