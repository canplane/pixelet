// @pxl/web/feature/map/ui/overlay/current-address.tsx

import { StyleSheet } from "react-native";

import { Motion } from "@platform/motion.tsx";
import type { LonLat } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { FADE } from "@/shared/ui/motion/transition.ts";
import { HaloText } from "@/shared/ui/halo.tsx";
import { color, textStyle, weight } from "@/shared/ui/theme.ts";

interface CurrentAddressProps {
  label: string | null;
}

/** Coordinates stand in where the center is in no place (sea, or not yet arrived). */
export function CurrentAddress(props: CurrentAddressProps) {
  const { label } = props;
  const position = useKernel((s) => s.map.center && coordinates(s.map.center));

  const text = label ?? position;
  if (!text) return null;

  return (
    <Motion key={label ?? ""} from={{ opacity: 0 }} to={{}} transition={FADE} style={styles.shrink}>
      <HaloText style={[styles.label, !label && styles.numbers]}>{text}</HaloText>
    </Motion>
  );
}

/** Four decimals: about eleven meters. */
function coordinates({ lat, lon }: LonLat): string {
  return `${Math.abs(lat).toFixed(4)}° ${lat < 0 ? "S" : "N"}, ${Math.abs(lon).toFixed(4)}° ${lon < 0 ? "W" : "E"}`;
}

const styles = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  label: { ...textStyle.body, fontWeight: weight.semibold, color: color.white },
  numbers: { fontVariant: ["tabular-nums"] },
});
