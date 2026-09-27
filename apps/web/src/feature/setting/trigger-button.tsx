// @pxl/web/feature/setting/trigger-button.tsx

import { useTranslation } from "react-i18next";

import { useAppContext } from "@/app/provider/context.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";

export function SettingsTriggerButton() {
  const { ui } = useAppContext();
  const { t } = useTranslation();

  return <CircularButton glyph="gearshape" onPress={ui.openSettingsModal} label={t("settings.trigger_aria_label")} />;
}
