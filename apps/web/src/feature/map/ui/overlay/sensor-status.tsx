// @pxl/web/feature/map/ui/overlay/sensor-status.tsx

import { Compass, Locate } from "lucide-react-native";
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
  // its way or failed takes a color.
  const icon = (Icon: typeof Locate, status: DevicePoseStatus) => (
    <View style={status === DevicePoseStatus.Idle && styles.off}>
      <OutlinedIcon icon={Icon} size={14} strokeWidth={2.5} color={statusColor(status)} />
    </View>
  );

  return (
    <View style={styles.row}>
      {icon(Locate, location)}
      {hasCompass && icon(Compass, orientation)}
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: 8 },
  off: { opacity: 0.5 },
});
