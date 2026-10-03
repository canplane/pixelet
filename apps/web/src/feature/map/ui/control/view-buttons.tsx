// @pxl/web/feature/map/ui/control/view-buttons.tsx

import { useTranslation } from "react-i18next";

import { Command, encode_setting_set } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";
import { ButtonStack } from "@/shared/ui/button-stack.tsx";

const MODE = SETTINGS.camera_mode;
const FREE = MODE.options.indexOf("free");
const ORBIT = MODE.options.indexOf("orbit");
const POI = SETTINGS.poi.key;

const set = (key: number, value: number) => kernel_command(Command.SETTING_SET, encode_setting_set({ key, value }));

/** How the map is seen: the person, which shows the mode it is in (an eye
 * first person, a figure third), and the places' badges, on or off. */
export function ViewButtons() {
  const { t } = useTranslation();
  const first = useKernel((s) => s.settings[MODE.key]) === FREE;
  const places = useKernel((s) => s.settings[POI]) === 1;
  return (
    <ButtonStack
      items={[
        {
          glyph: first ? "eye" : "figure.stand",
          label: t(first ? "map.controls.third_person" : "map.controls.first_person"),
          onPress: () => set(MODE.key, first ? ORBIT : FREE),
        },
        {
          glyph: "mappin",
          label: t(places ? "map.controls.hide_places" : "map.controls.show_places"),
          onPress: () => set(POI, places ? 0 : 1),
          isActive: places,
        },
      ]}
    />
  );
}
