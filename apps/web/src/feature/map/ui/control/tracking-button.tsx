// @pxl/web/feature/map/ui/control/tracking-button.tsx

import { useTranslation } from "react-i18next";

import { type GlyphName, Spinner } from "@platform/icon.tsx";
import { LABEL } from "@platform/palette.ts";
import { Motion } from "@platform/motion.tsx";
import { SETTLE } from "@/shared/ui/motion/transition.ts";
import { chrome } from "@/shared/ui/theme.ts";

import { DevicePoseStatus, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { kernel_tracking, useKernel } from "@/kernel/shell.ts";

import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";

/** As iPhone Maps draws its button. */
const PRESS: Record<Tracking, { glyph: GlyphName; label: string }> = {
  [Tracking.Recenter]: { glyph: "location", label: "map.controls.move_to_location" },
  [Tracking.Follow]: { glyph: "location.fill", label: "map.controls.start_compass_mode" },
  [Tracking.Unfollow]: { glyph: "location.north.line.fill", label: "map.controls.exit_compass_mode" },
};

export function TrackingButton() {
  const { t } = useTranslation();

  const status = useKernel((s) => s.location.status);
  const hasLocation = useKernel((s) => s.location.has_location);
  const following = useKernel((s) => s.is_follow_mode);
  const offCenter = useKernel((s) => s.map.is_off_center);

  const press = following ? Tracking.Unfollow : offCenter ? Tracking.Recenter : Tracking.Follow;
  const { glyph, label } = PRESS[press];

  return (
    <CircularButton
      onPress={() => void kernel_tracking(press)}
      isActive={following}
      disabled={status !== DevicePoseStatus.Ready || !hasLocation}
      label={t(label)}
    >
      {status === DevicePoseStatus.Loading ? <Spinner size={chrome.glyph} color={LABEL.primary} /> : (
        // Held still, as iPhone Maps holds it, the map being what turns with
        // the heading; it turns once into place as following starts, so the
        // change of mode is seen.
        <Motion to={{ rotate: 0 }} from={{ rotate: -90 }} replay={following ? "follow" : null} transition={SETTLE}>
          <Glyph glyph={glyph} size={chrome.glyph} color={LABEL.primary} />
        </Motion>
      )}
    </CircularButton>
  );
}
