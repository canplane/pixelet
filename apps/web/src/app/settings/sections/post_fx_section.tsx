// @pxl/web/app/settings/sections/post_fx_section.tsx

import { Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

export function PostFxSection() {
  const { t } = useTranslation();

  const show_ao = useSettingsStore((state) => state.show_ao);
  const toggle_ao = useSettingsStore((state) => state.toggle_ao);
  const show_bloom = useSettingsStore((state) => state.show_bloom);
  const toggle_bloom = useSettingsStore((state) => state.toggle_bloom);
  const show_tilt_shift = useSettingsStore((state) => state.show_tilt_shift);
  const toggle_tilt_shift = useSettingsStore((state) => state.toggle_tilt_shift);
  const show_chromatic_aberration = useSettingsStore((state) => state.show_chromatic_aberration);
  const toggle_chromatic_aberration = useSettingsStore((state) => state.toggle_chromatic_aberration);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Sparkles} caption={t("settings.post_fx.section_caption")}>
        {t("settings.post_fx.section_title")}
      </SectionTitle>
      <SettingRow
        label={t("settings.post_fx.chromatic_aberration.label")}
        description={t("settings.post_fx.chromatic_aberration.description")}
        active={show_chromatic_aberration}
        on_toggle={toggle_chromatic_aberration}
        cost="low"
      />
      <SettingRow
        label={t("settings.post_fx.tilt_shift.label")}
        description={t("settings.post_fx.tilt_shift.description")}
        active={show_tilt_shift}
        on_toggle={toggle_tilt_shift}
        cost="med"
      />
      {__IS_DEV__ && (
        <SettingRow
          label={t("settings.post_fx.bloom.label")}
          description={t("settings.post_fx.bloom.description")}
          active={show_bloom}
          on_toggle={toggle_bloom}
          cost="med"
          dev
        />
      )}
      <SettingRow
        label={t("settings.post_fx.ao.label")}
        description={t("settings.post_fx.ao.description")}
        active={show_ao}
        on_toggle={toggle_ao}
        cost="high"
      />
    </div>
  );
}
