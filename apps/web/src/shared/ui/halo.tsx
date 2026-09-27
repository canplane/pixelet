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
 * copies overlap it is no darker: a page and an iPhone fade a view as one
 * layer, and Android does when told to composite it offscreen.
 *
 * Its box is what it draws, rings and all: `halo.reach` of room on every
 * side of what it holds, which a caller counts in its gaps (`haloGap`).
 * Android cuts a layer at its view's bounds, so a ring drawn past its box,
 * whether the rings' own layer or an ancestor faded whole, was cut square.
 */
function Halo({ style, draw }: { style: StyleProp<ViewStyle>; draw: (ink?: string) => ReactNode }) {
  return (
    <View style={[style, styles.reach]}>
      {haloRings.map((px) => (
        <View key={px} aria-hidden needsOffscreenAlphaCompositing style={[StyleSheet.absoluteFill, { opacity: halo.ring }]}>
          {around(px).map(([x, y], i) => (
            <View key={i} style={[styles.copy, { transform: [{ translateX: x }, { translateY: y }] }]}>
              {draw(theme.black)}
            </View>
          ))}
        </View>
      ))}
      {
        // In a view of its own: on a page, a positioned box paints over an
        // unpositioned one whatever their order, and a bare glyph is an SVG
        // with no position, so the rings would lie over it and grey it.
      }
      <View>{draw()}</View>
    </View>
  );
}

/** The gap to lay out between two haloed things for `gap` between what they
 * draw: each box holds its halo's room. */
export function haloGap(gap: f64): f64 {
  return gap - 2 * halo.reach;
}

/** A glyph over the map in the halo its text has; crossed out, the strike is
 * haloed with it, which parts it from the strokes it crosses. */
export function HaloIcon({ struck, ...icon }: IconProps & { struck?: boolean }) {
  return <Halo style={{ width: icon.size + 2 * halo.reach, height: icon.size + 2 * halo.reach }} draw={(ink) => <Glyph {...icon} color={ink ?? icon.color} struck={struck} />} />;
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
  reach: { padding: halo.reach },
  copy: { position: "absolute", top: halo.reach, right: halo.reach, bottom: halo.reach, left: halo.reach },
});
