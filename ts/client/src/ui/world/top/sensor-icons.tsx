// @pxl/client/ui/world/top/sensor-icons.tsx

import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";

import { SensorStatus } from "@/abi/cmd.spec.ts";

import type { GlyphName } from "@/ui/component/icon.tsx";
import { haloGap, HaloIcon } from "./halo.tsx";
import { color } from "@/ui/component/theme.ts";
import * as device from "@/service/device.ts";
import { useCore } from "@/ui/store/watch.ts";

/** A sensor's status as a color: `tone.pending` while its reading is asked
 * for, `tone.failed` where it failed, else `base`. Each surface gives its own
 * tones, the world's glyphs paler than a sheet's. */
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
  const location = useCore((s) => s.location.status);
  const orientation = useCore((s) => s.orient.status);

  // Faded as one layer with its halo: faded apart, the halo grayed the glyph.
  const icon = (glyph: GlyphName, status: SensorStatus) => (
    <View needsOffscreenAlphaCompositing style={{ opacity: PRESENCE[status] }}>
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
      {device.handheld &&
        orientation !== SensorStatus.Unavailable &&
        icon("location.north.circle", orientation)}
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: haloGap(8) },
});
