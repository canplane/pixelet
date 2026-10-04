// @pxl/web/feature/setting/trigger-button.tsx

import { useTranslation } from "react-i18next";

import { openModal } from "@/app/modal.tsx";
import { CircularButton } from "@/shared/ui/circular-button.tsx";

export function SettingsTriggerButton() {
  const { t } = useTranslation();

  return <CircularButton glyph="gearshape" onPress={() => openModal("settings")} label={t("settings.trigger_aria_label")} />;
}
