// @pxl/client/ui/feature/world/top/sensor-icon.tsx

import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";

import {
  type SensorStatus,
  SENSOR_STATUS_ERROR,
  SENSOR_STATUS_IDLE,
  SENSOR_STATUS_LOADING,
  SENSOR_STATUS_READY,
  SENSOR_STATUS_UNAVAILABLE,
} from "@/abi/overlay.enum.ts";

import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { halo_gap, HaloIcon } from "./halo.tsx";
import { COLOR } from "@/ui/shared/component/theme.ts";
import * as device from "@/service/platform/device.ts";
import { useCore } from "@/ui/shared/watch.ts";

/** Each surface passes its own `tone`, because the glyphs over the world are
 * paler than a sheet's. */
export function status_color(
  status: SensorStatus,
  base: string,
  tone: { pending: string; failed: string },
): string {
  switch (status) {
    case SENSOR_STATUS_LOADING:
      return tone.pending;
    case SENSOR_STATUS_ERROR:
      return tone.failed;
    default:
      return base;
  }
}

const GLYPH_TONE = { pending: COLOR.pending_glyph, failed: COLOR.alarm_glyph };

const PRESENCE: Record<SensorStatus, f64> = {
  [SENSOR_STATUS_IDLE]: 0.35,
  [SENSOR_STATUS_LOADING]: 1,
  [SENSOR_STATUS_READY]: 0.7,
  [SENSOR_STATUS_ERROR]: 1,
  [SENSOR_STATUS_UNAVAILABLE]: 0.7,
};

export function SensorIcons() {
  const location = useCore((s) => s.location.status);
  const orientation = useCore((s) => s.orient.status);

  // The glyph and its halo fade as one layer. Faded apart, the halo would
  // show through and gray the glyph.
  const icon = (glyph: GlyphName, status: SensorStatus) => (
    <View needsOffscreenAlphaCompositing style={{ opacity: PRESENCE[status] }}>
      <HaloIcon
        glyph={glyph}
        size={12}
        color={status_color(status, COLOR.white, GLYPH_TONE)}
        struck={status === SENSOR_STATUS_ERROR}
      />
    </View>
  );

  return (
    <View style={styles.row}>
      {icon("location", location)}
      {device.HANDHELD &&
        orientation !== SENSOR_STATUS_UNAVAILABLE &&
        icon("location.north.circle", orientation)}
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: halo_gap(8) },
});
