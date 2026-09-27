// @pxl/web/feature/map/ui/overlay/current-address.tsx

import { StyleSheet, Text, View } from "react-native";

import { Motion } from "@platform/motion.tsx";
import type { LonLat } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { LEAVE } from "@/shared/ui/motion/transition.ts";
import { alpha, color, edge, textStyle, weight } from "@/shared/ui/theme.ts";

interface CurrentAddressProps {
  label: string | null;
}

/** The place under the map's center; a new place fades in over the room a
 * changed width has already set for it. Where the center is in no place —
 * over the sea, or before the place has arrived — its coordinates stand in,
 * as a map titles a spot with no name. */
export function CurrentAddress(props: CurrentAddressProps) {
  const { label } = props;
  const center = useKernel((s) => s.map.center);

  const text = label ?? (center && coordinates(center));
  if (!text) return null;

  return (
    <Motion key={label ?? ""} from={{ opacity: 0 }} to={{}} transition={{ ...LEAVE, durationMs: 200 }} style={styles.shrink}>
      <View style={styles.shrink}>
        {RIM.map(([x, y]) => (
          <Text key={`${x},${y}`} numberOfLines={1} aria-hidden style={[styles.label, !label && styles.numbers, styles.rim, { transform: [{ translateX: x }, { translateY: y }] }]}>
            {text}
          </Text>
        ))}
        <Text numberOfLines={1} style={[styles.label, !label && [styles.numbers, styles.unnamed]]}>
          {text}
        </Text>
      </View>
    </Motion>
  );
}

/** Four places: about eleven meters. */
function coordinates({ lat, lon }: LonLat): string {
  return `${Math.abs(lat).toFixed(4)}° ${lat < 0 ? "S" : "N"}, ${Math.abs(lon).toFixed(4)}° ${lon < 0 ? "W" : "E"}`;
}

/** The dark rim the scale bar's label is stroked with, as eight copies a pixel
 * out: a text has no stroke of its own, and a device takes one soft shadow. */
const RIM = [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]] as const;

const styles = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  label: { ...textStyle.body, fontWeight: weight.semibold, color: color.white },
  numbers: { fontVariant: ["tabular-nums"] },
  unnamed: { color: alpha(color.white, 0.8) },
  rim: { position: "absolute", top: 0, left: 0, right: 0, color: edge },
});
