// @pxl/client/shell/world/top/view-stack.tsx

import { useTranslation } from "react-i18next";

import { kernel_setting } from "@/port/shell.ts";
import { SETTING } from "@/shell/setting/schema.ts";
import { ButtonStack } from "./button-stack.tsx";
import { useKernel } from "@/shell/store/watch.ts";

const MODE = SETTING.camera_mode;
const FIRST_PERSON = MODE.options.first_person;
const ORBIT = MODE.options.orbit;
const PLACE = SETTING.place.key;

export function ViewStack() {
  const { t } = useTranslation();
  const first = useKernel((s) => s.settings[MODE.key]) === FIRST_PERSON;
  const places = useKernel((s) => s.settings[PLACE]) === 1;
  return (
    <ButtonStack
      items={[
        {
          glyph: first ? "eye" : "figure.stand",
          label: t(
            first
              ? "world.controls.third_person"
              : "world.controls.first_person",
          ),
          onPress: () => kernel_setting(MODE.key, first ? ORBIT : FIRST_PERSON),
        },
        {
          glyph: places ? "mappin.circle.fill" : "mappin.circle",
          label: t(
            places
              ? "world.controls.hide_places"
              : "world.controls.show_places",
          ),
          onPress: () => kernel_setting(PLACE, places ? 0 : 1),
          isActive: places,
        },
      ]}
    />
  );
}
