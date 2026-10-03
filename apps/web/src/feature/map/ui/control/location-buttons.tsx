// @pxl/web/feature/map/ui/control/location-buttons.tsx

import { useTranslation } from "react-i18next";

import { type GlyphName, Spinner } from "@platform/icon.tsx";
import { Motion } from "@platform/motion.tsx";
import { Command, DevicePoseStatus, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, kernel_tracking, useKernel } from "@/kernel/shell.ts";
import { CompassFace } from "@/feature/map/ui/control/compass.tsx";
import { ButtonStack } from "@/shared/ui/button-stack.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { SETTLE } from "@/shared/ui/motion/transition.ts";
import { chrome } from "@/shared/ui/theme.ts";

/** As MKUserTrackingButton draws it: an outline arrow away from where one
 * is, filled on it, and a north arrow in the tint while the map follows. */
const PRESS: Record<Tracking, { glyph: GlyphName; label: string }> = {
  [Tracking.Recenter]: { glyph: "location", label: "map.controls.move_to_location" },
  [Tracking.Follow]: { glyph: "location.fill", label: "map.controls.start_compass_mode" },
  [Tracking.Unfollow]: { glyph: "location.north.line.fill", label: "map.controls.exit_compass_mode" },
};

/** Where one is and which way is north, always shown, as Google Earth's. */
export function LocationButtons() {
  const { t } = useTranslation();
  const status = useKernel((s) => s.location.status);
  const hasLocation = useKernel((s) => s.location.has_location);
  const following = useKernel((s) => s.is_follow_mode);
  const offCenter = useKernel((s) => s.map.is_off_center);

  const press = following ? Tracking.Unfollow : offCenter ? Tracking.Recenter : Tracking.Follow;
  const { glyph, label } = PRESS[press];

  return (
    <ButtonStack
      items={[
        {
          label: t(label),
          onPress: () => void kernel_tracking(press),
          isActive: following,
          disabled: status !== DevicePoseStatus.Ready || !hasLocation,
          face: (tint) =>
            status === DevicePoseStatus.Loading ? <Spinner size={chrome.glyph} color={tint} /> : (
              // Turns once into place as following starts, so the change of mode is seen.
              <Motion to={{ rotate: 0 }} from={{ rotate: -90 }} replay={following ? "follow" : null} transition={SETTLE}>
                <Glyph glyph={glyph} size={chrome.glyph} color={tint} />
              </Motion>
            ),
        },
        {
          label: t("map.controls.reset_orientation"),
          onPress: () => kernel_command(Command.INPUT_COMPASS_RESET),
          // North up is the tracking button's to ask while following.
          disabled: following,
          face: () => <CompassFace />,
        },
      ]}
    />
  );
}
