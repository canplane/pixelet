// @pxl/web/app/settings/sections/quality_section.tsx

import { Gauge } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { SectionTitle, SettingRadioRow, SettingRow } from "@/app/settings/setting_row.tsx";

export function QualitySection() {
  const { t } = useTranslation();

  const aa_mode = useSettingsStore((state) => state.aa_mode);
  const set_aa_mode = useSettingsStore((state) => state.set_aa_mode);
  const low_spec = useSettingsStore((state) => state.low_spec);
  const toggle_low_spec = useSettingsStore((state) => state.toggle_low_spec);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Gauge} caption={t("settings.quality.section_caption")}>{t("settings.quality.section_title")}</SectionTitle>
      <SettingRow
        label={t("settings.quality.texture_pack.label")}
        description={t("settings.quality.texture_pack.description")}
        active={!low_spec}
        on_toggle={toggle_low_spec}
        cost="med"
      />
      {
        /* Grouped visually (bordered box + radios, not switches) since these
          three are mutually exclusive — one choice, not three independent
          toggles — unlike texture_pack above, which stands on its own.
          Below it because this group reaches "high" while that one is
          "med": cheapest first down the section. */
      }
      <div className="flex flex-col gap-0.5 -mx-3 rounded-[10px] border border-border p-1">
        <SettingRadioRow
          label={t("settings.quality.aa_mode.none.label")}
          description={t("settings.quality.aa_mode.none.description")}
          selected={aa_mode === "none"}
          on_select={() => set_aa_mode("none")}
        />
        <SettingRadioRow
          label={t("settings.quality.aa_mode.smaa.label")}
          description={t("settings.quality.aa_mode.smaa.description")}
          selected={aa_mode === "smaa"}
          on_select={() => set_aa_mode("smaa")}
          cost="med"
        />
        <SettingRadioRow
          label={t("settings.quality.aa_mode.supersample.label")}
          description={t("settings.quality.aa_mode.supersample.description")}
          selected={aa_mode === "supersample"}
          on_select={() => set_aa_mode("supersample")}
          cost="high"
        />
      </div>
    </div>
  );
}
