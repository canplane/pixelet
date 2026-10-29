// @pxl/client/shell/world/top/sensor-icons.tsx

import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";

import { SensorStatus } from "@/abi/cmd.spec.ts";

import type { GlyphName } from "@/shell/ui/icon.tsx";
import { haloGap, HaloIcon } from "./halo.tsx";
import { color } from "@/shell/ui/theme.ts";
import { bracket } from "@/bracket.tsx";
import { useKernel } from "@/shell/store/watch.ts";

/** A sensor's status as a color: `tone.pending` while its reading is asked
 * for, `tone.failed` where it failed, else `base`. Each surface gives its own
 * tones, the map's glyphs paler than a sheet's. */
export function statusColor(
  status: SensorStatus,
  base: string,
  tone: { pending: string; failed: string },
): string {
  switch (status) {
    case SensorStatus.Loading:
      return tone.pending;
    case SensorStatus.Error:
      return tone.failed;
    default:
      return base;
  }
}

const GLYPH_TONE = { pending: color.pendingGlyph, failed: color.alarmGlyph };

const PRESENCE: Record<SensorStatus, f64> = {
  [SensorStatus.Idle]: 0.35,
  [SensorStatus.Loading]: 1,
  [SensorStatus.Ready]: 0.7,
  [SensorStatus.Error]: 1,
  [SensorStatus.Unavailable]: 0.7,
};

export function SensorIcons() {
  const location = useKernel((s) => s.location.status);
  const orientation = useKernel((s) => s.orient.status);

  // Faded as one layer with its halo: faded apart, the halo grayed the glyph.
  const icon = (glyph: GlyphName, status: SensorStatus) => (
    <View
      needsOffscreenAlphaCompositing
      style={{ opacity: PRESENCE[status] }}
    >
      <HaloIcon
        glyph={glyph}
        size={12}
        color={statusColor(status, color.white, GLYPH_TONE)}
        struck={status === SensorStatus.Error}
      />
    </View>
  );

  return (
    <View style={styles.row}>
      {icon("location", location)}
      {bracket.device.handheld &&
        orientation !== SensorStatus.Unavailable &&
        icon("location.north.circle", orientation)}
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: haloGap(8) },
});
