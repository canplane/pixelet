// @pxl/client/shell/world/top/location-stack.tsx

import { useTranslation } from "react-i18next";

import { type GlyphName, Spinner } from "@/shell/ui/icon.tsx";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { LABEL } from "@/shell/ui/palette.ts";
import { Command, SensorStatus, Tracking } from "@/abi/cmd.spec.ts";
import { kernel_command, kernel_tracking } from "@/abi/shell.ts";
import { CompassFace } from "./compass.tsx";
import { ButtonStack } from "./button-stack.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { SETTLE } from "@/shell/ui/motion/transition.ts";
import { chrome, color } from "@/shell/ui/theme.ts";
import { useKernel } from "@/shell/store/watch.ts";

/** MKUserTrackingButton's glyphs. */
const PRESS: Record<Tracking, { glyph: GlyphName; label: string }> = {
  [Tracking.Recenter]: {
    glyph: "location",
    label: "world.controls.move_to_location",
  },
  [Tracking.Follow]: {
    glyph: "location.fill",
    label: "world.controls.start_compass_mode",
  },
  [Tracking.Unfollow]: {
    glyph: "location.north.line.fill",
    label: "world.controls.exit_compass_mode",
  },
};

export function LocationStack() {
  const { t } = useTranslation();
  const status = useKernel((s) => s.location.status);
  const hasLocation = useKernel((s) => s.location.has_location);
  const following = useKernel((s) => s.is_follow_mode);
  const offCenter = useKernel((s) => s.pivot.is_off_center);

  const press = following
    ? Tracking.Unfollow
    : offCenter
    ? Tracking.Recenter
    : Tracking.Follow;
  const { glyph, label } = PRESS[press];

  return (
    <ButtonStack
      isActive={following}
      items={[
        {
          label: t(label),
          onPress: () => void kernel_tracking(press),
          isActive: following,
          disabled: status !== SensorStatus.Ready || !hasLocation,
          face: status === SensorStatus.Loading
            ? <Spinner size={chrome.glyph} color={LABEL.primary} />
            : (
              // One turn as following starts.
              <Motion
                to={{ rotate: 0 }}
                from={{ rotate: -90 }}
                replay={following ? "follow" : null}
                transition={SETTLE}
              >
                <Glyph
                  glyph={glyph}
                  size={chrome.glyph}
                  color={following ? color.action : LABEL.primary}
                />
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
