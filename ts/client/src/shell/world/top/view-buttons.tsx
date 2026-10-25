// @pxl/client/shell/world/top/view-buttons.tsx

import { useTranslation } from "react-i18next";

import { kernel_setting } from "@/abi/shell.ts";
import { SETTINGS } from "@/shell/setting/schema.ts";
import { ButtonStack } from "./button-stack.tsx";
import { useKernel } from "@/shell/store/system.ts";

const MODE = SETTINGS.camera_mode;
const FREE = MODE.options.free;
const ORBIT = MODE.options.orbit;
const PLACE = SETTINGS.place.key;

export function ViewButtons() {
  const { t } = useTranslation();
  const first = useKernel((s) => s.settings[MODE.key]) === FREE;
  const places = useKernel((s) => s.settings[PLACE]) === 1;
  return (
    <ButtonStack
      items={[
        {
          glyph: first ? "eye" : "figure.stand",
          label: t(first ? "world.controls.third_person" : "world.controls.first_person"),
          onPress: () => kernel_setting(MODE.key, first ? ORBIT : FREE),
        },
        {
          glyph: places ? "mappin.circle.fill" : "mappin.circle",
          label: t(places ? "world.controls.hide_places" : "world.controls.show_places"),
          onPress: () => kernel_setting(PLACE, places ? 0 : 1),
          isActive: places,
        },
      ]}
    />
  );
}
