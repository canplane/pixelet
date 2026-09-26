// @pxl/web/shared/ui/outlined-icon.tsx

import type { LucideIcon } from "lucide-react-native";
import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { edge } from "@/shared/ui/theme.ts";

/** A glyph over the map on the dark rim the scale bar's lines are: the glyph
 * drawn twice, the rim a stroke wider behind it. A stroke is in the icon's
 * 24-unit box, so the rim's pixel on each side is scaled into it. */
export function OutlinedIcon({ icon: Icon, size, color, strokeWidth = 2 }: { icon: LucideIcon; size: f64; color: string; strokeWidth?: f64 }) {
  const rim = strokeWidth + (2 * 24) / size;
  return (
    <View style={{ width: size, height: size }}>
      <View style={styles.layer}>
        <Icon size={size} color={edge} strokeWidth={rim} />
      </View>
      <View style={styles.layer}>
        <Icon size={size} color={color} strokeWidth={strokeWidth} />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  layer: { position: "absolute", top: 0, left: 0 },
});
