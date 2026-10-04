// @pxl/web/shared/ui/glyph.tsx

import { StyleSheet, View } from "react-native";
import { Line, Svg } from "react-native-svg";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Icon, type IconProps, strokeOf } from "@platform/icon.tsx";

/**
 * A color at an opacity is drawn opaque and the glyph faded whole: an SVG
 * applies it per stroke, so crossings showed darker. `struck` crosses it out,
 * so a state never rests on color alone.
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

function opaque(color: string): [string, f64] {
  const m = /^rgba\(\s*(\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\s*\)$/.exec(color);
  return m ? [`rgb(${m[1]}, ${m[2]}, ${m[3]})`, Number(m[4])] : [color, 1];
}
