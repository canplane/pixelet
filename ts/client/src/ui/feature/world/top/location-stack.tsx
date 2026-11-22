// @pxl/client/ui/feature/world/top/location-stack.tsx

import { useTranslation } from "react-i18next";

import { type GlyphName, Spinner } from "@/ui/shared/component/icon.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import {
  type LocationMode,
  LOCATION_MODE_LOCATE,
  LOCATION_MODE_OFF,
  LOCATION_MODE_TRACK,
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

/** What the button shows in each location mode: the mode a press asks for,
 * its glyph and its label. */
const BUTTON: Record<
  LocationMode,
  { glyph: GlyphName; label: string; press: LocationMode }
> = {
  [LOCATION_MODE_OFF]: {
    glyph: "location",
    label: "world.controls.location_mode_off",
    press: LOCATION_MODE_LOCATE,
  },
  [LOCATION_MODE_LOCATE]: {
    glyph: "location.fill",
    label: "world.controls.location_mode_locate",
    press: LOCATION_MODE_TRACK,
  },
  [LOCATION_MODE_TRACK]: {
    glyph: "location.north.line.fill",
    label: "world.controls.location_mode_track",
    press: LOCATION_MODE_OFF,
  },
};

export function LocationStack() {
  const { t } = useTranslation();
  const status = useSensor((l) => l.state.status);
  const hasLocation = useSensor((l) => l.location !== null);
  const mode = useCore((s) => s.location_mode);
  const tracking = mode === LOCATION_MODE_TRACK;
  const { glyph, label, press } = BUTTON[mode];

  return (
    <ButtonStack
      isActive={tracking}
      items={[
        {
          label: t(label),
          onPress: () => void camera.setLocationMode(press),
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
