// @pxl/client/ui/component/glyph.tsx

import { StyleSheet, View } from "react-native";
import { Line, Svg } from "react-native-svg";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Icon, type IconProps } from "@/ui/component/icon.tsx";

/**
 * A glyph whose color has an alpha is drawn in the opaque color and faded as
 * a whole. An SVG applies a color's alpha to each stroke, so crossing strokes
 * would show darker. `struck` draws a line across the glyph, so that a state
 * never rests on color alone.
 */
export function Glyph({
  struck = false,
  ...icon
}: IconProps & { struck?: boolean }) {
  const [color, opacity] = opaque(icon.color);
  const props = { ...icon, color };
  return (
    <View style={{ width: icon.size, height: icon.size, opacity }}>
      <Icon {...props} />
      {struck && (
        <Svg
          style={StyleSheet.absoluteFill}
          width={icon.size}
          height={icon.size}
          viewBox="0 0 24 24"
        >
          <Line
            x1={2}
            y1={2}
            x2={22}
            y2={22}
            stroke={color}
            strokeWidth={stroke(icon.weight)}
            strokeLinecap="round"
          />
        </Svg>
      )}
    </View>
  );
}

/** The strike's width on the 24-unit box. It matches Phosphor's regular and
 * bold strokes, which are 16 and 24 units wide on Phosphor's 256-unit box. */
function stroke(weight: IconProps["weight"] = "regular"): f64 {
  return weight === "regular" ? 1.5 : 2.25;
}

function opaque(color: string): [string, f64] {
  const m = /^rgba\(\s*(\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\s*\)$/.exec(color);
  return m ? [`rgb(${m[1]}, ${m[2]}, ${m[3]})`, Number(m[4])] : [color, 1];
}
