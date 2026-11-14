// @pxl/client/ui/feature/world/pill-style.ts

import { StyleSheet } from "react-native";

export const pill_styles = StyleSheet.create({
  // `box-none` must come from a style sheet. React Native for Web draws an
  // inline `box-none` as `none`, which the content then inherits.
  box: { position: "absolute", pointerEvents: "box-none" },
  target: { position: "absolute" },
  content: {
    position: "absolute",
    left: 0,
    top: 0,
    flexDirection: "row",
    alignItems: "center",
    pointerEvents: "box-none",
  },
});
