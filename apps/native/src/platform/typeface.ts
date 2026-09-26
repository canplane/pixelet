// @pxl/native/platform/typeface.ts

import { Platform, type TextStyle } from "react-native";

/** The chrome's faces on a device: the system's own, and its monospace for
 * numbers that have to line up. */
export const FAMILY: string | undefined = undefined;
export const MONO: string | undefined = Platform.select({ ios: "Menlo", default: "monospace" });

/** A thin edge of `tint` around text, so it reads over any ground. */
export function textEdge(tint: string): TextStyle {
  return { textShadowColor: tint, textShadowOffset: { width: 0, height: 0 }, textShadowRadius: 2 };
}
