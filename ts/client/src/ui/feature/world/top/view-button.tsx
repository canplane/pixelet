// @pxl/client/ui/feature/world/top/view-button.tsx

import { useTranslation } from "react-i18next";

import { SETTING } from "@/ui/shared/setting.ts";
import { ButtonStack } from "./button-stack.tsx";
import { CircularButton } from "@/ui/shared/component/circular-button.tsx";
import { useCore } from "@/ui/shared/watch.ts";
import * as setting from "@/service/core/setting.ts";
import { CAMERA_STATE_IDLE } from "@/abi/overlay.enum.ts";

const MODE = SETTING.camera_mode;
const FIRST_PERSON = MODE.options.first_person;
const ORBIT = MODE.options.orbit;
const PLACE = SETTING.place.key;

/** The camera's mode, first person or the orbit; the button shows the one
 * it switches to. It is offered only while the camera is idle: the core
 * changes the mode then alone. */
export function ModeButton() {
  const { t } = useTranslation();
  const first = useCore((s) => s.settings[MODE.key]) === FIRST_PERSON;
  const idle = useCore((s) => s.camera) === CAMERA_STATE_IDLE;
  return (
    <CircularButton
      glyph={first ? "view.3d" : "figure.stand"}
      label={t(
        first ? "world.controls.third_person" : "world.controls.first_person",
      )}
      onPress={() => setting.set(MODE.key, first ? ORBIT : FIRST_PERSON)}
      disabled={!idle}
    />
  );
}

/** The places, shown or hidden. */
export function PlaceButton() {
  const { t } = useTranslation();
  const places = useCore((s) => s.settings[PLACE]) === 1;
  return (
    <ButtonStack
      items={[
        {
          glyph: places ? "mappin.circle.fill" : "mappin.circle",
          label: t(
            places
              ? "world.controls.hide_places"
              : "world.controls.show_places",
          ),
          onPress: () => setting.set(PLACE, places ? 0 : 1),
          isActive: places,
        },
      ]}
    />
  );
}
