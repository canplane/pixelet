// @pxl/web/shared/ui/halo.tsx

import type { ReactNode } from "react";
import { type StyleProp, StyleSheet, Text, type TextStyle, View, type ViewStyle } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import type { IconProps } from "@platform/icon.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { color as theme, halo, haloRings } from "@/shared/ui/theme.ts";

/**
 * What floats over the map, in the halo it stands in (theme.ts `halo`):
 * `draw` drawn in black at points around a circle as wide as each ring, and
 * over those as itself. Copies rather than a stroke, because nothing it
 * draws has one to widen on every platform — a device's text has none, and
 * an iPhone's symbol is an image — and copies take a word, a lucide glyph and
 * an SF Symbol alike: eight points a pixel out, more as the circle grows, so
 * no gap opens between two. Each ring is faded as a whole, so where the
 * copies overlap it is no darker.
 */
function Halo({ style, draw }: { style: StyleProp<ViewStyle>; draw: (ink?: string) => ReactNode }) {
  return (
    <View style={style}>
      {haloRings.map((px) => (
        <View key={px} aria-hidden style={[StyleSheet.absoluteFill, { opacity: halo.ring }]}>
          {around(px).map(([x, y], i) => (
            <View key={i} style={[StyleSheet.absoluteFill, { transform: [{ translateX: x }, { translateY: y }] }]}>
              {draw(theme.black)}
            </View>
          ))}
        </View>
      ))}
      {draw()}
    </View>
  );
}

/** A glyph over the map in the halo its text has; crossed out, the strike is
 * haloed with it, which parts it from the strokes it crosses. */
export function HaloIcon({ struck, ...icon }: IconProps & { struck?: boolean }) {
  return <Halo style={{ width: icon.size, height: icon.size }} draw={(ink) => <Glyph {...icon} color={ink ?? icon.color} struck={struck} />} />;
}

/** One line of text over the map in the halo. Every copy is cut short where
 * the line is. */
export function HaloText({ style, children }: { style: StyleProp<TextStyle>; children: ReactNode }) {
  return (
    <Halo
      style={styles.shrink}
      draw={(ink) => (
        <Text numberOfLines={1} style={[style, ink !== undefined && { color: ink }]}>
          {children}
        </Text>
      )}
    />
  );
}

/** Points on a circle of radius `px`, one a pixel apart along it. */
function around(px: f64): [f64, f64][] {
  const n = 8 * Math.ceil(px);
  return Array.from({ length: n }, (_, i) => {
    const a = (2 * Math.PI * i) / n;
    return [px * Math.cos(a), px * Math.sin(a)];
  });
}

const styles = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
});
