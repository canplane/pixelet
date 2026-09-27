// @pxl/native/platform/icon.tsx

import { SymbolView, type SymbolViewProps } from "expo-symbols";
import { ActivityIndicator, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import type { GlyphName, GlyphWeight, IconProps } from "../../../web/src/platform/web/icon.tsx";

export type { GlyphName, GlyphWeight, IconProps };
export { strokeOf } from "../../../web/src/platform/web/icon.tsx";

/** Every glyph the chrome names is a symbol an iPhone has: the names are
 * SF Symbols' own, and a name that is not one fails here. */
const symbol = (glyph: GlyphName): SymbolViewProps["name"] => glyph;

/** An iPhone's glyph, the SF Symbol itself (the page's is lucide's nearest,
 * platform/web/icon.tsx), in the weight the chrome asks for. */
export function Icon({ glyph, size, color, weight = "regular" }: IconProps) {
  return <SymbolView name={symbol(glyph)} size={size} tintColor={color} weight={weight} resizeMode="scaleAspectFit" />;
}

/** The system's activity indicator, which is what an iPhone shows while it
 * waits; it comes in one size, 20 points, and is scaled to the box. */
export function Spinner({ size, color }: { size: f64; color: string }) {
  return (
    <View style={{ width: size, height: size, alignItems: "center", justifyContent: "center" }}>
      <ActivityIndicator size="small" color={color} style={{ transform: [{ scale: size / 20 }] }} />
    </View>
  );
}
