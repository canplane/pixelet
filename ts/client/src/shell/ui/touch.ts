// @pxl/client/shell/ui/touch.ts

import { StyleSheet } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { chrome } from "@/shell/ui/theme.ts";

/** Whether a layer takes touches, from a sheet: RN-web draws an inline `box-none` as `none`, which every control under it inherits. */
export const touches = StyleSheet.create({
  pass: { pointerEvents: "box-none" },
  block: { pointerEvents: "none" },
});

/** What a control seen `size` high is grown by on each side so a press lands on `chrome.target`:
 * a box laid out around it, not `hitSlop`, which react-native-web does not honor. */
export function touchReach(size: f64): f64 {
  return Math.max(0, (chrome.target - size) / 2);
}
