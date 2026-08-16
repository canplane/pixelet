// @pxl/web/app/settings/sections/quality_section.tsx

import { Gauge } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useRenderSettingsStore } from "@/render/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

/** Base render-quality toggles — texture sampling and canvas resolution, independent of the effect stack. */
export function QualitySection() {
  const { t } = useTranslation();

  const low_spec = useRenderSettingsStore((state) => state.low_spec);
  const toggle_low_spec = useRenderSettingsStore((state) => state.toggle_low_spec);
  const high_dpr = useRenderSettingsStore((state) => state.high_dpr);
  const toggle_high_dpr = useRenderSettingsStore((state) => state.toggle_high_dpr);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Gauge}>{t("settings.quality.section_title")}</SectionTitle>
      <SettingRow
        label={t("settings.quality.texture_pack.label")}
        description={t("settings.quality.texture_pack.description")}
        active={!low_spec}
        on_toggle={toggle_low_spec}
        cost="med"
      />
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
