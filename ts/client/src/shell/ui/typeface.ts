// @pxl/client/shell/ui/typeface.ts

import { Platform, type TextStyle } from "react-native";

/** A page names the system's stack; a device's default is the system's. */
export const FAMILY: string | undefined = Platform.OS === "web"
  ? "-apple-system, BlinkMacSystemFont, system-ui, sans-serif"
  : undefined;

/** One CSS value on a page: react-native-web warns on the three a device takes. */
export function textEdge(tint: string): TextStyle {
  if (Platform.OS === "web") {
    return { textShadow: `0 0 2px ${tint}` } as TextStyle;
  }
  return {
    textShadowColor: tint,
    textShadowOffset: { width: 0, height: 0 },
    textShadowRadius: 2,
  };
}
