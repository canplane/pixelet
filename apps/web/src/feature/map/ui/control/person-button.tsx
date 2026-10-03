// @pxl/web/feature/map/ui/control/person-button.tsx

import { useTranslation } from "react-i18next";

import { Command, encode_setting_set } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";

const { key, options } = SETTINGS.camera_mode;
const FREE = options.indexOf("free");
const ORBIT = options.indexOf("orbit");

/** First person on and off: one glyph, lit while on, as the places button is. */
export function PersonButton() {
  const { t } = useTranslation();
  const first = useKernel((s) => s.settings[key]) === FREE;
  return (
    <CircularButton
      glyph="figure.walk"
      isActive={first}
      label={t(first ? "map.controls.third_person" : "map.controls.first_person")}
      onPress={() => kernel_command(Command.SETTING_SET, encode_setting_set({ key, value: first ? ORBIT : FREE }))}
    />
  );
}
