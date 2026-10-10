// @pxl/web/feature/world/ui/control/location-buttons.tsx

import { useTranslation } from "react-i18next";

import { Spinner } from "@platform/icon.tsx";
import { LABEL, STATUS } from "@platform/palette.ts";
import { Command, DevicePoseStatus } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, kernel_enable_location, useKernel } from "@/kernel/shell.ts";
import { CompassFace } from "@/feature/world/ui/control/compass.tsx";
import { Report } from "@/feature/world/ui/toast-controller.tsx";
import { messageKey } from "@/locale/message-key.ts";
import { ButtonStack } from "@/shared/ui/button-stack.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { toastFor, ToastTone } from "@/shared/ui/toast/store.ts";
import { chrome } from "@/shared/ui/theme.ts";

const SAID_MS = 3000;

/** The location's state is the button's: a failed one is pressed to ask again, and the press says why it failed. */
export function LocationButtons() {
  const { t } = useTranslation();
  const status = useKernel((s) => s.location.status);
  const hasLocation = useKernel((s) => s.location.has_location);
  const error = useKernel((s) => s.location.error);

  const press = () => {
    if (error !== null) {
      kernel_enable_location();
      toastFor(Report.Location, { message: t(messageKey(error)), tone: ToastTone.Error }, SAID_MS);
      return;
    }
    kernel_command(Command.INPUT_RECENTER);
  };
  const face = status === DevicePoseStatus.Loading
    ? <Spinner size={chrome.glyph} color={LABEL.primary} />
    : <Glyph glyph="location" size={chrome.glyph} color={error !== null ? STATUS.failed : LABEL.primary} />;

  return (
    <ButtonStack
      items={[
        {
          label: t("world.controls.move_to_location"),
          onPress: press,
          disabled: error === null && (status !== DevicePoseStatus.Ready || !hasLocation),
          face,
        },
        {
          label: t("world.controls.reset_orientation"),
          onPress: () => kernel_command(Command.INPUT_COMPASS_RESET),
          face: <CompassFace />,
        },
      ]}
    />
  );
}
