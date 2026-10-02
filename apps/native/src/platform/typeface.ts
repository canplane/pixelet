// @pxl/native/platform/typeface.ts

import { Platform, type TextStyle } from "react-native";

export const FAMILY: string | undefined = undefined;
export const MONO: string | undefined = Platform.select({ ios: "Menlo", default: "monospace" });

export function textEdge(tint: string): TextStyle {
  return { textShadowColor: tint, textShadowOffset: { width: 0, height: 0 }, textShadowRadius: 2 };
}
