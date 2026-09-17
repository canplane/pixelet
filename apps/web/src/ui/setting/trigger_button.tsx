// @pxl/web/ui/setting/trigger_button.tsx

import { Menu } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useAppContext } from "@/app/provider/context.ts";
import { CircularButton } from "@/kit/ui/shadcn/circular_button.tsx";

export function SettingsTriggerButton() {
  const { ui } = useAppContext();
  const { t } = useTranslation();

  return (
    <CircularButton
      className="w-12 h-12"
      shadow={false}
      onClick={ui.openSettingsModal}
      aria-label={t("settings.trigger_aria_label")}
      title={t("settings.trigger_aria_label")}
    >
      <Menu className="w-5 h-5 text-[#5B4636]/80" />
    </CircularButton>
  );
}
