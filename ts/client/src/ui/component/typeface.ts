// @pxl/client/ui/component/typeface.ts

import { Platform, type TextStyle } from "react-native";

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
