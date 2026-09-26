// @pxl/web/feature/map/ui/overlay/sensor-status.tsx

import { Compass, Locate } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, View } from "react-native";

import { DevicePoseStatus } from "@/kernel/abi/cmd.spec.ts";
import { kernel_enable_sensors, useKernel } from "@/kernel/shell.ts";

import { alpha, color } from "@/shared/ui/theme.ts";

function statusColor(status: DevicePoseStatus): string {
  switch (status) {
    case DevicePoseStatus.Ready:
      return color.emerald500;
    case DevicePoseStatus.Loading:
      return color.amber400;
    case DevicePoseStatus.Error:
      return color.red400;
    case DevicePoseStatus.Idle:
    default:
      return alpha(color.ink, 0.3);
  }
}

// DeviceOrientationEvent.requestPermission() only runs inside a direct user
// gesture on iOS Safari, and the kernel deliberately doesn't auto-start it
// there, so this control and the tracking button are the routes to that
// permission.
export function SensorStatus() {
  const { t } = useTranslation();

  const locationStatus = useKernel((s) => s.location.status);
  const orientationStatus = useKernel((s) => s.orientation.status);

  const canRequest: boolean = locationStatus !== DevicePoseStatus.Ready || orientationStatus !== DevicePoseStatus.Ready;

  const icon = (Icon: typeof Locate, status: DevicePoseStatus) => <Icon size={14} strokeWidth={2.5} color={statusColor(status)} />;
  const icons = (
    <>
      {icon(Locate, locationStatus)}
      {icon(Compass, orientationStatus)}
    </>
  );

  if (!canRequest) return <View style={styles.row}>{icons}</View>;

  return (
    <Pressable onPress={kernel_enable_sensors} hitSlop={8} accessibilityRole="button" accessibilityLabel={t("map.sensors.enable")} style={styles.row}>
      {icons}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: 8 },
});
