// @pxl/client/ui/feature/world/top/halo.tsx

import type { ReactNode } from "react";
import {
  type StyleProp,
  StyleSheet,
  Text,
  type TextStyle,
  View,
  type ViewStyle,
} from "react-native";
import { G, Path } from "react-native-svg";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { IconProps } from "@/ui/shared/component/icon.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import {
  color as theme,
  halo,
  halo_rings,
  text_scale,
} from "@/ui/shared/component/theme.ts";

/**
 * A dark halo made of black copies of `draw`, placed around circles of
 * growing radius. It cannot be a widened stroke, because a device's text and
 * an iPhone's symbol have no stroke to widen. Each ring fades as one layer,
 * so that copies that overlap are no darker. The box includes the rings,
 * because Android clips a layer at its view's bounds.
 */
function Halo({
  style,
  draw,
}: {
  style: StyleProp<ViewStyle>;
  draw: (ink?: string) => ReactNode;
}) {
  return (
    <View style={[style, styles.rings]}>
      {halo_rings.map((px) => (
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
        // The drawing gets its own view. A positioned box paints over an
        // unpositioned SVG, so the rings would otherwise gray the drawing.
      }
      <View>{draw()}</View>
    </View>
  );
}

export function halo_gap(gap: f64): f64 {
  return gap - 2 * halo.width;
}

export function HaloIcon({
  struck,
  ...icon
}: IconProps & { struck?: boolean }) {
  return (
    <Halo
      style={{
        width: icon.size + 2 * halo.width,
        height: icon.size + 2 * halo.width,
      }}
      draw={(ink) => (
        <Glyph {...icon} color={ink ?? icon.color} struck={struck} />
      )}
    />
  );
}

/** A white stroke over its halo, which the caller must draw inside an `Svg`.
 * The halo's rings are black strokes along `under`, which defaults to `d`. */
export function HaloPath({
  d,
  under = d,
  width,
  cap = "square",
}: {
  d: string;
  under?: string;
  width: f64;
  cap?: "butt" | "square";
}) {
  return (
    <>
      {halo_rings.map((px) => (
        <G key={px} opacity={halo.ring}>
          <Path
            d={under}
            stroke={theme.black}
            strokeWidth={width + 2 * px}
            fill="none"
            strokeLinecap={cap}
          />
        </G>
      ))}
      <Path
        d={d}
        stroke={theme.white}
        strokeWidth={width}
        fill="none"
        strokeLinecap={cap}
      />
    </>
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
          maxFontSizeMultiplier={text_scale.chrome}
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
  rings: { padding: halo.width },
  copy: {
    position: "absolute",
    top: halo.width,
    right: halo.width,
    bottom: halo.width,
    left: halo.width,
  },
});
