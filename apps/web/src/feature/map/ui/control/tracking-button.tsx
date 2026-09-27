// @pxl/web/feature/map/ui/control/tracking-button.tsx

import { useTranslation } from "react-i18next";
import { View } from "react-native";

import { Spinner } from "@platform/icon.tsx";
import { LABEL } from "@platform/palette.ts";

import { DevicePoseStatus, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { kernel_tracking, useKernel } from "@/kernel/shell.ts";

import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";

export function TrackingButton() {
  const { t } = useTranslation();

  const locationStatus = useKernel((s) => s.location.status);
  const hasLocation = useKernel((s) => s.location.has_location);
  const heading = useKernel((s) => s.orientation.heading);
  const azimuth = useKernel((s) => s.azimuth);
  const isFollowMode = useKernel((s) => s.is_follow_mode);
  const isOffCenter = useKernel((s) => s.map.is_off_center);

  const canShowLocationControl: boolean = isOffCenter && !isFollowMode;
  const isEnabled: boolean = locationStatus === DevicePoseStatus.Ready && hasLocation;
  const isLoading: boolean = locationStatus === DevicePoseStatus.Loading;

  const icon = () => {
    if (isLoading) {
      return <Spinner size={20} color={LABEL.primary} />;
    }
    // As iPhone Maps draws its button: an outline arrow to go back to where
    // one is, a filled one while there, north's line while the map follows
    // the heading.
    if (canShowLocationControl) return <Glyph glyph="location" size={20} color={LABEL.primary} />;
    if (isFollowMode) {
      const finalRotationDeg = (heading ?? 0) - (azimuth * 180) / Math.PI;
      // Turned by the view around it, in React Native's own transform: a CSS
      // string on the icon is the page's SVG, and a device's SVG refuses it.
      return (
        <View style={{ transform: [{ rotate: `${finalRotationDeg}deg` }] }}>
          <Glyph glyph="location.north.line.fill" size={20} color={LABEL.primary} />
        </View>
      );
    }
    return <Glyph glyph="location.fill" size={20} color={LABEL.primary} />;
  };

  const title = canShowLocationControl ? t("map.controls.move_to_location") : isFollowMode ? t("map.controls.exit_compass_mode") : t("map.controls.start_compass_mode");

  return (
    <CircularButton
      onPress={() => void kernel_tracking(canShowLocationControl ? Tracking.Recenter : isFollowMode ? Tracking.Unfollow : Tracking.Follow)}
      isActive={isFollowMode}
      disabled={!isEnabled || isLoading}
      label={title}
    >
      {icon()}
    </CircularButton>
  );
}
