// @pxl/web/feature/setting/trigger-button.tsx

import { Menu } from "lucide-react-native";
import { useTranslation } from "react-i18next";

import { useAppContext } from "@/app/provider/context.ts";
import { CircularButton } from "@/shared/ui/circular-button.tsx";
import { alpha, color } from "@/shared/ui/theme.ts";

export function SettingsTriggerButton() {
  const { ui } = useAppContext();
  const { t } = useTranslation();

  return (
    <CircularButton onPress={ui.openSettingsModal} label={t("settings.trigger_aria_label")}>
      <Menu size={20} color={alpha(color.ink, 0.8)} />
    </CircularButton>
  );
}
