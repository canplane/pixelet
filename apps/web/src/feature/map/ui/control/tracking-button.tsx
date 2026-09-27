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
  const has_location = useKernel((s) => s.location.has_location);
  const heading = useKernel((s) => s.orientation.heading);
  const azimuth = useKernel((s) => s.azimuth);
  const is_follow_mode = useKernel((s) => s.is_follow_mode);
  const is_off_center = useKernel((s) => s.map.is_off_center);

  const canShowLocationControl: boolean = is_off_center && !is_follow_mode;
  const isEnabled: boolean = locationStatus === DevicePoseStatus.Ready && has_location;
  const isLoading: boolean = locationStatus === DevicePoseStatus.Loading;

  const icon = () => {
    if (isLoading) {
      return <Spinner size={20} color={LABEL.primary} />;
    }
    // As iPhone Maps draws its button: an outline arrow to go back to where
    // one is, a filled one while there, north's line while the map follows
    // the heading.
    if (canShowLocationControl) return <Glyph glyph="location" size={20} color={LABEL.primary} />;
    if (is_follow_mode) {
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
