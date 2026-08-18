// @pxl/web/app/settings/settings_trigger_button.tsx

import { Menu } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useAppContext } from "@/app/providers/app_context.ts";
import { CircularButton } from "@/shared/ui/theme/cozy/circular_button.tsx";

export function SettingsTriggerButton() {
  const { ui } = useAppContext();
  const { t } = useTranslation();

  return (
    <CircularButton
      className="w-[42px] h-[42px]"
      dark
      shadow={false}
      onClick={ui.open_settings_modal}
      aria-label={t("settings.trigger_aria_label")}
      title={t("settings.trigger_aria_label")}
    >
      <Menu className="w-6 h-6 text-white/80" />
    </CircularButton>
  );
}
