// @pxl/web/app/settings/sections/quality_section.tsx

import { Gauge } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useRenderSettingsStore } from "@/render/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

export function QualitySection() {
  const { t } = useTranslation();

  const high_dpr = useRenderSettingsStore((state) => state.high_dpr);
  const toggle_high_dpr = useRenderSettingsStore((state) => state.toggle_high_dpr);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Gauge} caption={t("settings.quality.section_caption")}>{t("settings.quality.section_title")}</SectionTitle>
      <SettingRow
        label={t("settings.quality.high_dpr.label")}
        description={t("settings.quality.high_dpr.description")}
        active={high_dpr}
        on_toggle={toggle_high_dpr}
        cost="high"
      />
    </div>
  );
}
