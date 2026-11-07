// @pxl/client/ui/world/pill-style.ts

/** What a pill lays out alike on every platform (pill.tsx, pill.ios.tsx). */

import { StyleSheet } from "react-native";

export const pillStyles = StyleSheet.create({
  // From the sheet: an inline `box-none` is drawn as a `none` the content inherits.
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
