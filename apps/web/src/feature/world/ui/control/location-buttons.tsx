// @pxl/web/feature/world/ui/control/location-buttons.tsx

import { useTranslation } from "react-i18next";

import { type GlyphName, Spinner } from "@platform/icon.tsx";
import { Motion } from "@platform/motion.tsx";
import { LABEL } from "@platform/palette.ts";
import { Command, DevicePoseStatus, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, kernel_tracking, useKernel } from "@/kernel/shell.ts";
import { CompassFace } from "@/feature/world/ui/control/compass.tsx";
import { ButtonStack } from "@/shared/ui/button-stack.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { SETTLE } from "@/shared/ui/motion/transition.ts";
import { chrome, color } from "@/shared/ui/theme.ts";

/** MKUserTrackingButton's glyphs. */
const PRESS: Record<Tracking, { glyph: GlyphName; label: string }> = {
  [Tracking.Recenter]: { glyph: "location", label: "world.controls.move_to_location" },
  [Tracking.Follow]: { glyph: "location.fill", label: "world.controls.start_compass_mode" },
  [Tracking.Unfollow]: { glyph: "location.north.line.fill", label: "world.controls.exit_compass_mode" },
};

export function LocationButtons() {
  const { t } = useTranslation();
  const status = useKernel((s) => s.location.status);
  const hasLocation = useKernel((s) => s.location.has_location);
  const following = useKernel((s) => s.is_follow_mode);
  const offCenter = useKernel((s) => s.focus.is_off_center);

  const press = following ? Tracking.Unfollow : offCenter ? Tracking.Recenter : Tracking.Follow;
  const { glyph, label } = PRESS[press];

  return (
    <ButtonStack
      isActive={following}
      items={[
        {
          label: t(label),
          onPress: () => void kernel_tracking(press),
          isActive: following,
          disabled: status !== DevicePoseStatus.Ready || !hasLocation,
          face: status === DevicePoseStatus.Loading ? <Spinner size={chrome.glyph} color={LABEL.primary} /> : (
            // One turn as following starts.
            <Motion to={{ rotate: 0 }} from={{ rotate: -90 }} replay={following ? "follow" : null} transition={SETTLE}>
              <Glyph glyph={glyph} size={chrome.glyph} color={following ? color.action : LABEL.primary} />
            </Motion>
          ),
        },
        {
          label: t("world.controls.reset_orientation"),
          onPress: () => kernel_command(Command.INPUT_COMPASS_RESET),
          disabled: following,
          face: <CompassFace />,
        },
      ]}
    />
  );
}
