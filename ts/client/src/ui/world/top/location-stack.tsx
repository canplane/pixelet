// @pxl/client/ui/world/top/location-stack.tsx

import { useTranslation } from "react-i18next";

import { type GlyphName, Spinner } from "@/ui/component/icon.tsx";
import { Motion } from "@/ui/component/motion/motion.tsx";
import { LABEL } from "@/ui/component/palette.ts";
import { SensorStatus, Tracking } from "@/abi/cmd.spec.ts";
import { CompassFace } from "./compass.tsx";
import { ButtonStack } from "./button-stack.tsx";
import { Glyph } from "@/ui/component/glyph.tsx";
import { SETTLE } from "@/ui/component/motion/transition.ts";
import { chrome, color } from "@/ui/component/theme.ts";
import { useCore } from "@/ui/store/watch.ts";
import * as camera from "@/port/camera.ts";

/** The glyphs follow MapKit's MKUserTrackingButton. */
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
  const status = useCore((s) => s.location.status);
  const hasLocation = useCore((s) => s.location.has_location);
  const following = useCore((s) => s.is_follow_mode);
  const offCenter = useCore((s) => s.pivot.is_off_center);

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
          onPress: () => void camera.tracking(press),
          isActive: following,
          disabled: status !== SensorStatus.Ready || !hasLocation,
          face:
            status === SensorStatus.Loading ? (
              <Spinner size={chrome.glyph} color={LABEL.primary} />
            ) : (
              // The glyph turns once as following starts.
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
          onPress: camera.north_up,
          disabled: following,
          face: <CompassFace />,
        },
      ]}
    />
  );
}
