// @pxl/client/ui/shared/component/typeface.web.ts

import type { TextStyle } from "react-native";

/** The web takes this as one CSS value, because react-native-web warns on the
 * three props the native file takes (typeface.native.ts). */
export function text_halo(tint: string): TextStyle {
  return { textShadow: `0 0 2px ${tint}` } as TextStyle;
}
