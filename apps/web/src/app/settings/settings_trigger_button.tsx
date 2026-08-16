// @pxl/web/app/settings/settings_trigger_button.tsx

import { Settings } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useAppContext } from "@/app/providers/app_context.ts";
import { CircularButton } from "@/shared/ui/theme/cozy/circular_button.tsx";

/**
 * Always-visible top-left entry point for the settings modal (rendering
 * effects / debug / license) — never __IS_DEV__-gated, since the license
 * section it opens onto is a required data-source disclosure in production
 * too, not just a dev diagnostic.
 */
export function SettingsTriggerButton() {
  const { ui } = useAppContext();
  const { t } = useTranslation();

  return (
    <CircularButton
      className="w-11 h-11"
      dark
      shadow={false}
      onClick={ui.open_settings_modal}
      aria-label={t("settings.trigger_aria_label")}
      title={t("settings.trigger_aria_label")}
    >
      <Settings className="w-5 h-5 text-white/80" />
    </CircularButton>
  );
}
