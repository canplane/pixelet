// @pxl/web/feature/map/ui/overlay/current-address.tsx

import { StyleSheet, Text, useWindowDimensions, View } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { LEAVE } from "@/shared/ui/motion/transition.ts";
import { color, edge, textStyle, weight } from "@/shared/ui/theme.ts";

interface CurrentAddressProps {
  label: string | null;
}

/** The place under the map's center; a new place fades in over the room a
 * changed width has already set for it. */
export function CurrentAddress(props: CurrentAddressProps) {
  const { label } = props;
  const { width } = useWindowDimensions();

  if (!label) return null;

  return (
    <Motion key={label} from={{ opacity: 0 }} to={{}} transition={{ ...LEAVE, durationMs: 200 }} style={styles.shrink}>
      <View style={[styles.shrink, { maxWidth: width * 0.4 }]}>
        {RIM.map(([x, y]) => (
          <Text key={`${x},${y}`} numberOfLines={1} aria-hidden style={[styles.label, styles.rim, { transform: [{ translateX: x }, { translateY: y }] }]}>
            {label}
          </Text>
        ))}
        <Text numberOfLines={1} style={styles.label}>
          {label}
        </Text>
      </View>
    </Motion>
  );
}

/** The dark rim the scale bar's label is stroked with, as eight copies a pixel
 * out: a text has no stroke of its own, and a device takes one soft shadow. */
const RIM = [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]] as const;

const styles = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  label: { ...textStyle.footnote, fontWeight: weight.semibold, color: color.white },
  rim: { position: "absolute", top: 0, left: 0, right: 0, color: edge },
});
