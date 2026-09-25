// @pxl/native/platform/glass.tsx

/**
 * A floating control's ground, on a device: the system's glass where it has
 * one — Liquid Glass, from iOS 26, which bends and lights what is under it,
 * the map included — and the control's fill where it has none, as on the web.
 */

import { GlassView, isLiquidGlassAvailable } from "expo-glass-effect";
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

/** Asked once: the OS does not gain or lose glass while the app runs. */
const GLASS = isLiquidGlassAvailable();

export function Glass({ fill, tint, interactive, style, children }: GlassProps) {
  if (!GLASS) return <View style={[style, { backgroundColor: fill }]}>{children}</View>;
  return (
    <GlassView style={style} tintColor={tint} isInteractive={interactive}>
      {children}
    </GlassView>
  );
}
