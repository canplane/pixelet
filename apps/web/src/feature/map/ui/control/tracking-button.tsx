// @pxl/web/feature/map/ui/control/tracking-button.tsx

import { Compass, Locate, Navigation } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { ActivityIndicator } from "react-native";

import { DevicePoseStatus, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { kernel_tracking, useKernel } from "@/kernel/shell.ts";

import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { alpha, color } from "@/shared/ui/theme.ts";

export function TrackingButton() {
  const { t } = useTranslation();

  const locationStatus = useKernel((s) => s.location.status);
  const has_location = useKernel((s) => s.location.has_location);
  const heading = useKernel((s) => s.orientation.heading);
  const azimuth = useKernel((s) => s.azimuth);
  const is_follow_mode = useKernel((s) => s.is_follow_mode);
  const is_off_center = useKernel((s) => s.map.is_off_center);

  const canShowLocationControl: boolean = is_off_center && !is_follow_mode;
  const isEnabled: boolean = locationStatus === DevicePoseStatus.Ready && has_location;
  const isLoading: boolean = locationStatus === DevicePoseStatus.Loading;

  const icon = () => {
    if (isLoading) return <ActivityIndicator size="small" color={alpha(color.ink, 0.4)} />;
    if (canShowLocationControl) return <Locate size={18} color={alpha(color.ink, 0.6)} />;
    if (is_follow_mode) {
      const finalRotationDeg = (heading ?? 0) - (azimuth * 180) / Math.PI;
      return <Compass size={22} color={color.amber500} style={{ transform: `rotate(${finalRotationDeg}deg)` }} />;
    }
    return <Navigation size={20} color={alpha(color.ink, 0.8)} />;
  };

  const title = canShowLocationControl ? t("map.controls.move_to_location") : is_follow_mode ? t("map.controls.exit_compass_mode") : t("map.controls.start_compass_mode");

  return (
    <CircularButton
      onPress={() => void kernel_tracking(canShowLocationControl ? Tracking.Recenter : is_follow_mode ? Tracking.Unfollow : Tracking.Follow)}
      isActive={is_follow_mode}
      disabled={!isEnabled || isLoading}
      label={title}
    >
      {icon()}
    </CircularButton>
  );
}
