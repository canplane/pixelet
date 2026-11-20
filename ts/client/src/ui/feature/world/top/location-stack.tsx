// @pxl/client/ui/feature/world/top/location-stack.tsx

import { useTranslation } from "react-i18next";

import { type GlyphName, Spinner } from "@/ui/shared/component/icon.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import {
  type TrackingMode,
  TRACKING_MODE_FOLLOW,
  TRACKING_MODE_FOLLOW_WITH_HEADING,
  TRACKING_MODE_NONE,
} from "@/abi/types.enum.ts";
import {
  SENSOR_STATUS_LOADING,
  SENSOR_STATUS_READY,
} from "@/service/core/sensor/reading.ts";
import { ButtonStack } from "./button-stack.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { SETTLE } from "@/ui/shared/component/motion/transition.ts";
import { CHROME, COLOR } from "@/ui/shared/component/theme.ts";
import { useCore, useSensor } from "@/ui/shared/watch.ts";
import * as camera from "@/service/core/camera.ts";

/** What the button shows in each tracking mode, its glyph and its label the
 * mode's own as Apple's Maps names its button, and the mode a press asks
 * for, the modes in turn. */
const BUTTON: Record<
  TrackingMode,
  { glyph: GlyphName; label: string; press: TrackingMode }
> = {
  [TRACKING_MODE_NONE]: {
    glyph: "location",
    label: "world.controls.tracking_off",
    press: TRACKING_MODE_FOLLOW,
  },
  [TRACKING_MODE_FOLLOW]: {
    glyph: "location.fill",
    label: "world.controls.tracking_on",
    press: TRACKING_MODE_FOLLOW_WITH_HEADING,
  },
  [TRACKING_MODE_FOLLOW_WITH_HEADING]: {
    glyph: "location.north.line.fill",
    label: "world.controls.tracking_on_with_heading",
    press: TRACKING_MODE_NONE,
  },
};

export function LocationStack() {
  const { t } = useTranslation();
  const status = useSensor((l) => l.state.status);
  const hasLocation = useSensor((l) => l.location !== null);
  const mode = useCore((s) => s.tracking_mode);
  const tracking = mode !== TRACKING_MODE_NONE;
  const { glyph, label, press } = BUTTON[mode];

  return (
    <ButtonStack
      isActive={tracking}
      items={[
        {
          label: t(label),
          onPress: () => void camera.track(press),
          isActive: tracking,
          disabled: status !== SENSOR_STATUS_READY || !hasLocation,
          face:
            status === SENSOR_STATUS_LOADING ? (
              <Spinner size={CHROME.glyph} color={LABEL.primary} />
            ) : (
              <Motion
                to={{ rotate: 0 }}
                from={{ rotate: -90 }}
                replay={tracking ? "tracking" : null}
                transition={SETTLE}
              >
                <Glyph
                  glyph={glyph}
                  size={CHROME.glyph}
                  color={tracking ? COLOR.action : LABEL.primary}
                />
              </Motion>
            ),
        },
      ]}
    />
  );
}
