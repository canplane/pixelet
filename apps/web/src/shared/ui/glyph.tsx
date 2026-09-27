// @pxl/web/shared/ui/glyph.tsx

import { StyleSheet, View } from "react-native";
import { Line, Svg } from "react-native-svg";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Icon, type IconProps, strokeOf } from "@platform/icon.tsx";

/**
 * A glyph of the chrome (platform/web/icon.tsx, and an iPhone's symbol), and
 * what the chrome does to any glyph, done here once for every platform's.
 *
 * A color at an opacity is drawn opaque and the glyph faded whole: an SVG
 * takes an opacity in its color stroke by stroke, so where a glyph's strokes
 * meet or cross — an eraser's edge on its body, a strike on what it strikes
 * — the two opacities stacked and the joint showed darker than the rest.
 *
 * `struck` draws it crossed out, corner to corner as a `.slash` symbol is,
 * for something that failed or is off where the symbol has no crossed-out
 * form of its own: the state does not rest on color alone. The stroke is the
 * glyph's own weight, in the same 24-unit box.
 */
export function Glyph({ struck = false, ...icon }: IconProps & { struck?: boolean }) {
  const [color, opacity] = opaque(icon.color);
  const drawn = { ...icon, color };
  return (
    <View style={{ width: icon.size, height: icon.size, opacity }}>
      <Icon {...drawn} />
      {struck && (
        <Svg style={StyleSheet.absoluteFill} width={icon.size} height={icon.size} viewBox="0 0 24 24">
          <Line x1={2} y1={2} x2={22} y2={22} stroke={color} strokeWidth={strokeOf(icon.weight)} strokeLinecap="round" />
        </Svg>
      )}
    </View>
  );
}

/** A color as its opaque self and the opacity it was at: `rgba()`, which
 * theme.ts's `alpha` writes, split; anything else, a system's color among
 * them, as it is. */
function opaque(color: string): [string, f64] {
  const m = /^rgba\(\s*(\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\s*\)$/.exec(color);
  return m ? [`rgb(${m[1]}, ${m[2]}, ${m[3]})`, Number(m[4])] : [color, 1];
}
