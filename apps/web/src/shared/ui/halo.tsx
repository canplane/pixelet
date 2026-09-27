// @pxl/web/shared/ui/halo.tsx

import type { LucideIcon } from "lucide-react-native";
import type { ReactNode } from "react";
import { type StyleProp, StyleSheet, Text, type TextStyle, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { color as theme, halo } from "@/shared/ui/theme.ts";

/** A glyph over the map in the halo its text has (theme.ts `halo`). Each ring
 * is faded as a whole, so where the icon's own strokes cross it is no darker.
 * A stroke is in the icon's 24-unit box, so a ring's pixels are scaled into
 * it, and the rings are drawn in a box widened by the widest ring, since an
 * icon clips what it draws to its own box. */
export function HaloIcon({ icon: Icon, size, color, strokeWidth = 2 }: { icon: LucideIcon; size: f64; color: string; strokeWidth?: f64 }) {
  const unit = 24 / size;
  const out = Math.max(...halo.rings);
  const ring = { top: -out, left: -out, opacity: halo.ring };
  const viewBox = `${-out * unit} ${-out * unit} ${24 + 2 * out * unit} ${24 + 2 * out * unit}`;
  return (
    <View style={{ width: size, height: size }}>
      {halo.rings.map((px) => (
        <View key={px} style={[styles.layer, ring]}>
          <Icon size={size + 2 * out} viewBox={viewBox} color={theme.black} strokeWidth={strokeWidth + 2 * px * unit} />
        </View>
      ))}
      <View style={styles.layer}>
        <Icon size={size} color={color} strokeWidth={strokeWidth} />
      </View>
    </View>
  );
}

/** One line of text over the map in the halo (theme.ts `halo`). A text has no
 * stroke on a device, so a ring is the line drawn in black at points around a
 * circle as wide as the ring, which together cover what a stroke that wide
 * would: eight points a pixel out, more as the circle grows, so no gap opens
 * between two. Every copy wraps and ends as the line does. */
export function HaloText({ style, children }: { style: StyleProp<TextStyle>; children: ReactNode }) {
  return (
    <View style={styles.shrink}>
      {halo.rings.map((px) => (
        <View key={px} aria-hidden style={[StyleSheet.absoluteFill, { opacity: halo.ring }]}>
          {around(px).map(([x, y], i) => (
            <Text key={i} numberOfLines={1} style={[style, styles.under, styles.ink, { transform: [{ translateX: x }, { translateY: y }] }]}>
              {children}
            </Text>
          ))}
        </View>
      ))}
      <Text numberOfLines={1} style={style}>
        {children}
      </Text>
    </View>
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
  layer: { position: "absolute", top: 0, left: 0 },
  shrink: { flexShrink: 1, minWidth: 0 },
  under: { position: "absolute", top: 0, left: 0, right: 0 },
  ink: { color: theme.black },
});
