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
import { G, Path, Svg } from "react-native-svg";

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { IconProps } from "@/ui/shared/component/icon.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import {
  COLOR as theme,
  HALO,
  HALO_RINGS,
  TEXT_SCALE_CAP,
} from "@/ui/shared/component/theme.ts";

/**
 * A dark halo made of black copies of `draw`, placed around circles of
 * growing radius. It cannot be a widened stroke, because the native text and
 * an iPhone's symbol have no stroke to widen. Each ring fades as one layer,
 * so that copies that overlap are no darker. The box includes the rings,
 * because Android clips a layer at its view's bounds, and a margin as deep
 * takes them back out, so that the layout sees only what is drawn. The whole
 * fades as one layer by `opacity`, so that the halo does not show through
 * and gray the drawing.
 */
function Halo({
  style,
  opacity,
  draw,
}: {
  style?: StyleProp<ViewStyle>;
  opacity?: f64;
  draw: (ink?: string) => ReactNode;
}) {
  return (
    <View
      needsOffscreenAlphaCompositing={opacity !== undefined}
      style={[style, STYLE.rings, opacity !== undefined && { opacity }]}
    >
      {HALO_RINGS.map((px) => (
        <View
          key={px}
          aria-hidden
          needsOffscreenAlphaCompositing
          style={[StyleSheet.absoluteFill, { opacity: HALO.ring }]}
        >
          {around(px).map(([x, y], i) => (
            <View
              key={i}
              style={[
                STYLE.copy,
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

export function HaloIcon({
  struck,
  opacity,
  ...icon
}: IconProps & { struck?: boolean; opacity?: f64 }) {
  return (
    <Halo
      opacity={opacity}
      style={{
        width: icon.size + 2 * HALO.width,
        height: icon.size + 2 * HALO.width,
      }}
      draw={(ink) => (
        <Glyph {...icon} color={ink ?? icon.color} struck={struck} />
      )}
    />
  );
}

/** The room an `HaloSvg` keeps around its drawing: a halo, and half of a
 * stroke two wide. */
const PAD_PX = HALO.width + 1;

/** An `Svg` drawn in units that start at its top left, with room around for
 * the halo of what is drawn near its edges, taken back out by a margin as
 * `Halo`'s is. */
export function HaloSvg({
  width,
  height,
  children,
}: {
  width: f64;
  height: f64;
  children: ReactNode;
}) {
  const [w, h] = [width + 2 * PAD_PX, height + 2 * PAD_PX];
  return (
    <View style={STYLE.svg}>
      <Svg width={w} height={h} viewBox={`${-PAD_PX} ${-PAD_PX} ${w} ${h}`}>
        {children}
      </Svg>
    </View>
  );
}

/** A white stroke over its halo, which the caller must draw inside an
 * `HaloSvg`.
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
      {HALO_RINGS.map((px) => (
        <G key={px} opacity={HALO.ring}>
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
      style={STYLE.shrink}
      draw={(ink) => (
        <Text
          numberOfLines={1}
          maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
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

const STYLE = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  rings: { padding: HALO.width, margin: -HALO.width },
  svg: { margin: -PAD_PX },
  copy: {
    position: "absolute",
    top: HALO.width,
    right: HALO.width,
    bottom: HALO.width,
    left: HALO.width,
  },
});
