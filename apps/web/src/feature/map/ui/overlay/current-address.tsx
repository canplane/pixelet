// @pxl/web/feature/map/ui/overlay/current-address.tsx

import { StyleSheet } from "react-native";

import { Motion } from "@platform/motion.tsx";
import type { LonLat } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { LEAVE } from "@/shared/ui/motion/transition.ts";
import { HaloText } from "@/shared/ui/halo.tsx";
import { color, textStyle, weight } from "@/shared/ui/theme.ts";

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
      <HaloText style={[styles.label, !label && styles.numbers]}>{text}</HaloText>
    </Motion>
  );
}

/** Four places: about eleven meters. */
function coordinates({ lat, lon }: LonLat): string {
  return `${Math.abs(lat).toFixed(4)}° ${lat < 0 ? "S" : "N"}, ${Math.abs(lon).toFixed(4)}° ${lon < 0 ? "W" : "E"}`;
}

const styles = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  label: { ...textStyle.body, fontWeight: weight.semibold, color: color.white },
  numbers: { fontVariant: ["tabular-nums"] },
});
