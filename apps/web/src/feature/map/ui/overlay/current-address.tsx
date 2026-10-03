// @pxl/web/feature/map/ui/overlay/current-address.tsx

import { useTranslation } from "react-i18next";
import { StyleSheet } from "react-native";

import { Motion } from "@platform/motion.tsx";
import type { LonLat } from "@/kernel/abi/cmd.spec.ts";
import { deepestRank } from "@/feature/map/geocode/depth.ts";
import { assembleLabel } from "@/feature/map/geocode/label.ts";
import { useKernel } from "@/kernel/shell.ts";
import { FADE } from "@/shared/ui/motion/transition.ts";
import { HaloText } from "@/shared/ui/halo.tsx";
import { color, textStyle, weight } from "@/shared/ui/theme.ts";

/** Coordinates stand in where the center is in no place (sea, or not yet arrived). */
export function CurrentAddress() {
  const label = usePlaceLabel();
  const position = useKernel((s) => s.map.center && coordinates(s.map.center));

  const text = label ?? position;
  if (!text) return null;

  return (
    <Motion key={label ?? ""} from={{ opacity: 0 }} to={{}} transition={FADE} style={styles.shrink}>
      <HaloText style={[styles.label, !label && styles.numbers]}>{text}</HaloText>
    </Motion>
  );
}

/** Assembled per snapshot, so a language switch needs no refetch. */
function usePlaceLabel(): string | null {
  const { i18n } = useTranslation();
  return useKernel((s) => {
    const deepest = deepestRank(s.m_per_px, s.map.center?.lat ?? 0);
    const parts = s.place.filter((p) => p.rank <= deepest);
    return parts.length === 0 ? null : assembleLabel(parts.map((p) => ({ name: p.name, names: Object.fromEntries(p.names.map((n) => [n.lang, n.name])) })), i18n.language);
  });
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
