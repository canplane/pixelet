// @pxl/web/platform/web/glass.tsx

/**
 * A floating control's ground, on the web: its fill. A device draws the
 * system's glass in this place where it has one (Liquid Glass, from iOS 26);
 * a page has no system glass, so the control stays the solid it was designed
 * as.
 */

import type { ReactNode } from "react";
import { type ColorValue, type StyleProp, View, type ViewStyle } from "react-native";

export interface GlassProps {
  /** The ground where there is no glass. */
  fill: ColorValue;
  /** The glass's tint where there is glass: a dark strip stays dark. */
  tint?: ColorValue;
  /** Whether the glass answers a touch, as a control's does. */
  interactive?: boolean;
  style?: StyleProp<ViewStyle>;
  children?: ReactNode;
}

export function Glass({ fill, style, children }: GlassProps) {
  return <View style={[style, { backgroundColor: fill }]}>{children}</View>;
}
