// @pxl/client/ui/shared/component/touch.ts

import { StyleSheet } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { chrome } from "@/ui/shared/component/theme.ts";

/** Whether a layer takes touches. These styles must come from a style sheet,
 * not inline: react-native-web draws an inline `box-none` as `none`, which
 * every control under the layer then inherits. */
export const touches = StyleSheet.create({
  pass: { pointerEvents: "box-none" },
  block: { pointerEvents: "none" },
});

/** How far a control drawn `size` high grows on each side, so that the area a
 * press lands on is `chrome.target` high. The caller lays a box out around
 * the control rather than using `hitSlop`, which react-native-web does not
 * honor. */
export function touch_outset(size: f64): f64 {
  return Math.max(0, (chrome.target - size) / 2);
}
