// @pxl/web/feature/map/ui/control/places-button.tsx

import { useTranslation } from "react-i18next";

import { Command, encode_setting_set } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";

const { key } = SETTINGS.poi;

/** The places' badges on and off, as a map's layer button turns them. */
export function PlacesButton() {
  const { t } = useTranslation();
  const shown = useKernel((s) => s.settings[key]) === 1;
  return (
    <CircularButton
      glyph="mappin"
      isActive={shown}
      label={t(shown ? "map.controls.hide_places" : "map.controls.show_places")}
      onPress={() => kernel_command(Command.SETTING_SET, encode_setting_set({ key, value: shown ? 0 : 1 }))}
    />
  );
}
