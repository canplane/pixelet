// @pxl/client/ui/shared/component/typeface.native.ts

import type { TextStyle } from "react-native";

export function textEdge(tint: string): TextStyle {
  return {
    textShadowColor: tint,
    textShadowOffset: { width: 0, height: 0 },
    textShadowRadius: 2,
  };
}
