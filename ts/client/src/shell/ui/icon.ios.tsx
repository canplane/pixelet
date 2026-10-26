// @pxl/client/shell/ui/icon.ios.tsx

import { SymbolView, type SymbolViewProps } from "expo-symbols";
import { ActivityIndicator, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { GlyphName, IconProps } from "./icon.tsx";

export type { GlyphName, IconProps };

/** A name that is not an SF Symbol fails to type-check here. */
const symbol = (glyph: GlyphName): SymbolViewProps["name"] => glyph;

export function Icon({ glyph, size, color, weight = "regular" }: IconProps) {
  return (
    <SymbolView
      name={symbol(glyph)}
      size={size}
      tintColor={color}
      weight={weight}
      resizeMode="scaleAspectFit"
    />
  );
}

/** The system indicator comes in one size, 20 pt, so it is scaled to the box. */
export function Spinner({ size, color }: { size: f64; color: string }) {
  return (
    <View
      style={{
        width: size,
        height: size,
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <ActivityIndicator
        size="small"
        color={color}
        style={{ transform: [{ scale: size / 20 }] }}
      />
    </View>
  );
}
