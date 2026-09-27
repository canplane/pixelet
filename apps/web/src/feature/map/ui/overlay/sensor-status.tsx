// @pxl/web/feature/map/ui/overlay/sensor-status.tsx

import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";

import { DevicePoseStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";

import { HANDHELD } from "@platform/core.ts";
import type { GlyphName } from "@platform/icon.tsx";
import { haloGap, HaloIcon } from "@/shared/ui/halo.tsx";
import { color } from "@/shared/ui/theme.ts";

/** Where the two sensors stand: read by the glyphs over the map and by the
 * search panel's card that turns the location on. */
export function useSensors() {
  const location = useKernel((s) => s.location.status);
  const orientation = useKernel((s) => s.orientation.status);
  // A compass is only a hand-held device's to have: a desktop's is shown
  // nowhere, and one the device turned out not to have (Unavailable) likewise.
  const hasCompass = HANDHELD && orientation !== DevicePoseStatus.Unavailable;
  return { location, orientation, hasCompass };
}

/** A sensor's glyph by its state: its color and how present it is. The row is
 * a subtitle under the address, so a sensor that is on stays quiet, and one
 * not yet on is half as present, as iOS draws what is off. Only a sensor on
 * its way or failed takes a color, drawn in full: it has something to say. */
const LOOK: Record<DevicePoseStatus, { color: string; presence: f64 }> = {
  [DevicePoseStatus.Idle]: { color: color.white, presence: 0.35 },
  [DevicePoseStatus.Loading]: { color: color.pendingGlyph, presence: 1 },
  [DevicePoseStatus.Ready]: { color: color.white, presence: 0.7 },
  [DevicePoseStatus.Error]: { color: color.alarmGlyph, presence: 1 },
  [DevicePoseStatus.Unavailable]: { color: color.white, presence: 0.7 },
};

/** The sensors' state, over the map: shown, not pressed — they are turned on
 * from the search panel. */
export function SensorStatus() {
  const { location, orientation, hasCompass } = useSensors();

  // The glyph is faded whole, its halo with it, as one layer: faded apart,
  // the halo showed through the glyph and greyed it. A failed one is also
  // crossed out, so the state does not rest on color alone.
  const icon = (glyph: GlyphName, status: DevicePoseStatus) => (
    <View needsOffscreenAlphaCompositing style={{ opacity: LOOK[status].presence }}>
      <HaloIcon glyph={glyph} size={12} color={LOOK[status].color} struck={status === DevicePoseStatus.Error} />
    </View>
  );

  return (
    <View style={styles.row}>
      {icon("location", location)}
      {hasCompass && icon("location.north.circle", orientation)}
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: haloGap(8) },
});
