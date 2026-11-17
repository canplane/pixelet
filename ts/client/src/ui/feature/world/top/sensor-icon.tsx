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
} from "@/service/core/sensor/reading.ts";

import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { HaloIcon } from "./halo.tsx";
import { COLOR } from "@/ui/shared/component/theme.ts";
import * as device from "@/service/platform/device.ts";
import { useSensor } from "@/ui/shared/watch.ts";

/** Each surface passes its own `tone`, because the glyphs over the world are
 * paler than a sheet's. */
export function statusColor(
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

const GLYPH_TONE = { pending: COLOR.pendingGlyph, failed: COLOR.alarmGlyph };

const PRESENCE: Record<SensorStatus, f64> = {
  [SENSOR_STATUS_IDLE]: 0.35,
  [SENSOR_STATUS_LOADING]: 1,
  [SENSOR_STATUS_READY]: 0.7,
  [SENSOR_STATUS_ERROR]: 1,
  [SENSOR_STATUS_UNAVAILABLE]: 0.7,
};

export function SensorIcons() {
  const location = useSensor((l) => l.state.status);
  const orientation = useSensor((_, o) => o.status);

  const icon = (glyph: GlyphName, status: SensorStatus) => (
    <HaloIcon
      glyph={glyph}
      size={12}
      color={statusColor(status, COLOR.white, GLYPH_TONE)}
      struck={status === SENSOR_STATUS_ERROR}
      opacity={PRESENCE[status]}
    />
  );

  return (
    <View style={STYLE.row}>
      {icon("location", location)}
      {device.HANDHELD &&
        orientation !== SENSOR_STATUS_UNAVAILABLE &&
        icon("location.north.circle", orientation)}
    </View>
  );
}

const STYLE = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: 8 },
});
