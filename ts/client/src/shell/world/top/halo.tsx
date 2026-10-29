// @pxl/client/shell/world/top/halo.tsx

import type { ReactNode } from "react";
import {
  type StyleProp,
  StyleSheet,
  Text,
  type TextStyle,
  View,
  type ViewStyle,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { IconProps } from "@/shell/ui/icon.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import {
  color as theme,
  halo,
  haloRings,
  textScale,
} from "@/shell/ui/theme.ts";

/**
 * A dark halo from black copies of `draw` around circles, not a stroke: a
 * device's text and an iPhone's symbol have none to widen. Each ring fades as
 * one layer so overlaps are no darker. The box includes the rings, since
 * Android cuts a layer at its view's bounds.
 */
function Halo({
  style,
  draw,
}: {
  style: StyleProp<ViewStyle>;
  draw: (ink?: string) => ReactNode;
}) {
  return (
    <View style={[style, styles.reach]}>
      {haloRings.map((px) => (
        <View
          key={px}
          aria-hidden
          needsOffscreenAlphaCompositing
          style={[StyleSheet.absoluteFill, { opacity: halo.ring }]}
        >
          {around(px).map(([x, y], i) => (
            <View
              key={i}
              style={[
                styles.copy,
                {
                  transform: [{ translateX: x }, { translateY: y }],
                },
              ]}
            >
              {draw(theme.black)}
            </View>
          ))}
        </View>
      ))}
      {
        // Its own view: a positioned box paints over an unpositioned SVG, which the
        // rings would grey.
      }
      <View>{draw()}</View>
    </View>
  );
}

export function haloGap(gap: f64): f64 {
  return gap - 2 * halo.reach;
}

export function HaloIcon({
  struck,
  ...icon
}: IconProps & { struck?: boolean }) {
  return (
    <Halo
      style={{
        width: icon.size + 2 * halo.reach,
        height: icon.size + 2 * halo.reach,
      }}
      draw={(ink) => (
        <Glyph {...icon} color={ink ?? icon.color} struck={struck} />
      )}
    />
  );
}

export function HaloText({
  style,
  children,
}: {
  style: StyleProp<TextStyle>;
  children: ReactNode;
}) {
  return (
    <Halo
      style={styles.shrink}
      draw={(ink) => (
        <Text
          numberOfLines={1}
          maxFontSizeMultiplier={textScale.chrome}
          style={[style, ink !== undefined && { color: ink }]}
        >
          {children}
        </Text>
      )}
    />
  );
}

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
  copy: {
    position: "absolute",
    top: halo.reach,
    right: halo.reach,
    bottom: halo.reach,
    left: halo.reach,
  },
});
