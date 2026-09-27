// @pxl/web/feature/map/ui/overlay/sensor-status.tsx

import { Compass, createLucideIcon, Locate, LocateOff, type LucideIcon } from "lucide-react-native";
import { StyleSheet, View } from "react-native";

import { DevicePoseStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";

import { HANDHELD } from "@platform/core.ts";
import { STATUS } from "@platform/palette.ts";
import { OutlinedIcon } from "@/shared/ui/outlined-icon.tsx";
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

/** Lucide has no crossed-out compass, so this is its Compass drawn the way
 * its other "-off" icons are: the 2,2 → 22,22 slash, and the circle split
 * into two arcs that stop 12° short of where the slash crosses it (at 45°
 * and 225°), so the slash does not touch the outline. */
const CompassOff = createLucideIcon("compass-off", [
  ["path", { d: "M6.55 3.61A10 10 0 0 1 20.39 17.45", key: "arc-upper" }],
  ["path", { d: "M17.45 20.39A10 10 0 0 1 3.61 6.55", key: "arc-lower" }],
  ["path", { d: "m16.24 7.76-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z", key: "needle" }],
  ["path", { d: "m2 2 20 20", key: "slash" }],
]);

function statusColor(status: DevicePoseStatus): string {
  switch (status) {
    case DevicePoseStatus.Loading:
      return STATUS.pending;
    case DevicePoseStatus.Error:
      return color.alarmGlyph;
    default:
      return color.white;
  }
}

/** The sensors' state, over the map: shown, not pressed — they are turned on
 * from the search panel. */
export function SensorStatus() {
  const { location, orientation, hasCompass } = useSensors();

  // A sensor not yet on is the glyph faded whole, its rim with it, as iOS
  // draws what is off; one that is on is plain white, and only a sensor on
  // its way or failed takes a color. A failed one is also crossed out, so
  // the state does not rest on color alone.
  const icon = (Icon: LucideIcon, Failed: LucideIcon, status: DevicePoseStatus) => (
    <View style={status === DevicePoseStatus.Idle && styles.off}>
      <OutlinedIcon icon={status === DevicePoseStatus.Error ? Failed : Icon} size={14} strokeWidth={2.5} color={statusColor(status)} />
    </View>
  );

  return (
    <View style={styles.row}>
      {icon(Locate, LocateOff, location)}
      {hasCompass && icon(Compass, CompassOff, orientation)}
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: 8 },
  off: { opacity: 0.5 },
});
