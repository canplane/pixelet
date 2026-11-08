// @pxl/client/ui/component/typeface.native.ts

import type { TextStyle } from "react-native";

/** A thin edge of `tint` around text, drawn as a shadow with no offset, in the
 * three props a device takes. */
export function textEdge(tint: string): TextStyle {
  return {
    textShadowColor: tint,
    textShadowOffset: { width: 0, height: 0 },
    textShadowRadius: 2,
  };
}
