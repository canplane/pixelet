// @pxl/client/ui/component/typeface.ts

import type { TextStyle } from "react-native";

/** A thin edge of `tint` around text, drawn as a shadow with no offset. A
 * page takes it as one CSS value, because react-native-web warns on the three
 * props a device takes (typeface.native.ts). */
export function textEdge(tint: string): TextStyle {
  return { textShadow: `0 0 2px ${tint}` } as TextStyle;
}
