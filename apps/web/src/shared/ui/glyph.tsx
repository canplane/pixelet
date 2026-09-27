// @pxl/web/shared/ui/glyph.tsx

import { StyleSheet, View } from "react-native";
import { Line, Svg } from "react-native-svg";

import { Icon, type IconProps, strokeOf } from "@platform/icon.tsx";

/**
 * A glyph of the chrome (platform/web/icon.tsx, and an iPhone's symbol), and
 * what the chrome does to any glyph, done here once for every platform's.
 * `struck` draws it crossed out, corner to corner as a `.slash` symbol is, for
 * something that failed or is off where the symbol has no crossed-out form
 * of its own: the state does not rest on color alone. The stroke is the
 * glyph's own weight, in the same 24-unit box.
 */
export function Glyph({ struck = false, ...icon }: IconProps & { struck?: boolean }) {
  if (!struck) return <Icon {...icon} />;
  return (
    <View style={{ width: icon.size, height: icon.size }}>
      <Icon {...icon} />
      <Svg style={StyleSheet.absoluteFill} width={icon.size} height={icon.size} viewBox="0 0 24 24">
        <Line x1={2} y1={2} x2={22} y2={22} stroke={icon.color} strokeWidth={strokeOf(icon.weight)} strokeLinecap="round" />
      </Svg>
    </View>
  );
}
