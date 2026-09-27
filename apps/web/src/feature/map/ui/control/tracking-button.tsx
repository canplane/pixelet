// @pxl/web/feature/map/ui/control/tracking-button.tsx

import { useTranslation } from "react-i18next";
import { View } from "react-native";

import { type GlyphName, Spinner } from "@platform/icon.tsx";
import { LABEL } from "@platform/palette.ts";
import { chrome } from "@/shared/ui/theme.ts";

import { DevicePoseStatus, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { kernel_tracking, useKernel } from "@/kernel/shell.ts";

import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";

/** What a press does, and what the button shows for it — as iPhone Maps
 * draws its button: an outline arrow to go back to where one is, a filled
 * one while there, north's line while the map follows the heading. */
const PRESS: Record<Tracking, { glyph: GlyphName; label: string }> = {
  [Tracking.Recenter]: { glyph: "location", label: "map.controls.move_to_location" },
  [Tracking.Follow]: { glyph: "location.fill", label: "map.controls.start_compass_mode" },
  [Tracking.Unfollow]: { glyph: "location.north.line.fill", label: "map.controls.exit_compass_mode" },
};

const GLYPH = chrome.glyph;

export function TrackingButton() {
  const { t } = useTranslation();

  const status = useKernel((s) => s.location.status);
  const hasLocation = useKernel((s) => s.location.has_location);
  const heading = useKernel((s) => s.orientation.heading);
  const azimuth = useKernel((s) => s.azimuth);
  const following = useKernel((s) => s.is_follow_mode);
  const offCenter = useKernel((s) => s.map.is_off_center);

  const press = following ? Tracking.Unfollow : offCenter ? Tracking.Recenter : Tracking.Follow;
  const { glyph, label } = PRESS[press];
  // North's line points where the device heads, on the map as it is turned.
  const turn = following ? (heading ?? 0) - (azimuth * 180) / Math.PI : 0;

  return (
    <CircularButton
      onPress={() => void kernel_tracking(press)}
      isActive={following}
      disabled={status !== DevicePoseStatus.Ready || !hasLocation}
      label={t(label)}
    >
      {status === DevicePoseStatus.Loading ? <Spinner size={GLYPH} color={LABEL.primary} /> : (
        // Turned by the view around it, in React Native's own transform: a
        // CSS string on the icon is the page's SVG, and a device's SVG
        // refuses it.
        <View style={{ transform: [{ rotate: `${turn}deg` }] }}>
          <Glyph glyph={glyph} size={GLYPH} color={LABEL.primary} />
        </View>
      )}
    </CircularButton>
  );
}
