// @pxl/client/ui/shared/component/typeface.ts

import type { TextStyle } from "react-native";

/** A page takes this as one CSS value, because react-native-web warns on the
 * three props a device takes (typeface.native.ts). */
export function textEdge(tint: string): TextStyle {
  return { textShadow: `0 0 2px ${tint}` } as TextStyle;
}
