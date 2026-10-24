// @pxl/client/shell/world/overlay/sensor-status.tsx

import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";

import { DevicePoseStatus } from "@/host/abi/cmd.spec.ts";
import { useKernel } from "@/host/shell.ts";

import { HANDHELD } from "@/bracket.tsx";
import type { GlyphName } from "@/shell/ui/icon.tsx";
import { haloGap, HaloIcon } from "@/shell/ui/halo.tsx";
import { color } from "@/shell/ui/theme.ts";

const LOOK: Record<DevicePoseStatus, { color: string; presence: f64 }> = {
  [DevicePoseStatus.Idle]: { color: color.white, presence: 0.35 },
  [DevicePoseStatus.Loading]: { color: color.pendingGlyph, presence: 1 },
  [DevicePoseStatus.Ready]: { color: color.white, presence: 0.7 },
  [DevicePoseStatus.Error]: { color: color.alarmGlyph, presence: 1 },
  [DevicePoseStatus.Unavailable]: { color: color.white, presence: 0.7 },
};

export function SensorStatus() {
  const location = useKernel((s) => s.location.status);
  const orientation = useKernel((s) => s.orientation.status);

  // Faded as one layer with its halo: faded apart, the halo greyed the glyph.
  const icon = (glyph: GlyphName, status: DevicePoseStatus) => (
    <View needsOffscreenAlphaCompositing style={{ opacity: LOOK[status].presence }}>
      <HaloIcon glyph={glyph} size={12} color={LOOK[status].color} struck={status === DevicePoseStatus.Error} />
    </View>
  );

  return (
    <View style={styles.row}>
      {icon("location", location)}
      {HANDHELD && orientation !== DevicePoseStatus.Unavailable && icon("location.north.circle", orientation)}
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: haloGap(8) },
});
