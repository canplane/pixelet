// @pxl/client/ui/feature/world/top/location-stack.tsx

import { useTranslation } from "react-i18next";

import { type GlyphName, Spinner } from "@/ui/shared/component/icon.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import {
  type Tracking,
  TRACKING_FOLLOW,
  TRACKING_RECENTER,
  TRACKING_UNFOLLOW,
} from "@/abi/camera.enum.ts";
import {
  SENSOR_STATUS_LOADING,
  SENSOR_STATUS_READY,
} from "@/abi/overlay.enum.ts";
import { CompassFace } from "./compass.tsx";
import { ButtonStack } from "./button-stack.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { SETTLE } from "@/ui/shared/component/motion/transition.ts";
import { chrome, color } from "@/ui/shared/component/theme.ts";
import { useCore } from "@/ui/shared/watch.ts";
import * as camera from "@/service/core/camera.ts";

/** The glyphs follow MapKit's MKUserTrackingButton. */
const PRESS: Record<Tracking, { glyph: GlyphName; label: string }> = {
  [TRACKING_RECENTER]: {
    glyph: "location",
    label: "world.controls.move_to_location",
  },
  [TRACKING_FOLLOW]: {
    glyph: "location.fill",
    label: "world.controls.start_compass_mode",
  },
  [TRACKING_UNFOLLOW]: {
    glyph: "location.north.line.fill",
    label: "world.controls.exit_compass_mode",
  },
};

export function LocationStack() {
  const { t } = useTranslation();
  const status = useCore((s) => s.location.status);
  const has_location = useCore((s) => s.location.has_location);
  const following = useCore((s) => s.is_follow_mode);
  const is_off_center = useCore((s) => s.pivot.is_off_center);

  const press = following
    ? TRACKING_UNFOLLOW
    : is_off_center
      ? TRACKING_RECENTER
      : TRACKING_FOLLOW;
  const { glyph, label } = PRESS[press];

  return (
    <ButtonStack
      isActive={following}
      items={[
        {
          label: t(label),
          on_press: () => void camera.tracking(press),
          is_active: following,
          disabled: status !== SENSOR_STATUS_READY || !has_location,
          face:
            status === SENSOR_STATUS_LOADING ? (
              <Spinner size={chrome.glyph} color={LABEL.primary} />
            ) : (
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
          on_press: camera.north_up,
          disabled: following,
          face: <CompassFace />,
        },
      ]}
    />
  );
}
