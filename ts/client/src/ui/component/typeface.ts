// @pxl/client/ui/component/typeface.ts

import { Platform, type TextStyle } from "react-native";

/** A thin edge of `tint` around text, drawn as a shadow with no offset. A
 * page takes it as one CSS value, because react-native-web warns on the three
 * props a device takes. */
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
