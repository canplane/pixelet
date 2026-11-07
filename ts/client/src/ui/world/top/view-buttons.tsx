// @pxl/client/ui/world/top/view-buttons.tsx

import { useTranslation } from "react-i18next";

import { SETTING } from "@/ui/setting/schema.ts";
import { ButtonStack } from "./button-stack.tsx";
import { useCore } from "@/ui/store/watch.ts";
import * as setting from "@/port/setting.ts";

const MODE = SETTING.camera_mode;
const FIRST_PERSON = MODE.options.first_person;
const ORBIT = MODE.options.orbit;
const PLACE = SETTING.place.key;

/** The camera's mode and the places, a circle each: the one changes how the
 * world is seen, the other what it shows. */
export function ViewButtons() {
  const { t } = useTranslation();
  const first = useCore((s) => s.settings[MODE.key]) === FIRST_PERSON;
  const places = useCore((s) => s.settings[PLACE]) === 1;
  return (
    <>
      <ButtonStack
        items={[
          {
            glyph: first ? "eye" : "figure.stand",
            label: t(
              first
                ? "world.controls.third_person"
                : "world.controls.first_person",
            ),
            onPress: () => setting.set(MODE.key, first ? ORBIT : FIRST_PERSON),
          },
        ]}
      />
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
    </>
  );
}
