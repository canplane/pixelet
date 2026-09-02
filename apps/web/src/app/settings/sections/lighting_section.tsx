// @pxl/web/app/settings/sections/lighting_section.tsx

import { Sun } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

export function LightingSection() {
  const { t } = useTranslation();

  const show_shadow_map = useSettingsStore((state) => state.show_shadow_map);
  const toggle_shadow_map = useSettingsStore((state) => state.toggle_shadow_map);
  const show_rim_light = useSettingsStore((state) => state.show_rim_light);
  const toggle_rim_light = useSettingsStore((state) => state.toggle_rim_light);
  const show_god_rays = useSettingsStore((state) => state.show_god_rays);
  const toggle_god_rays = useSettingsStore((state) => state.toggle_god_rays);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Sun} caption={t("settings.lighting.section_caption")}>
        {t("settings.lighting.section_title")}
      </SectionTitle>
      <SettingRow
        label={t("settings.lighting.rim_light.label")}
        description={t("settings.lighting.rim_light.description")}
        active={show_rim_light}
        on_toggle={toggle_rim_light}
        cost="low"
      />
      {__IS_DEV__ && (
        <SettingRow
          label={t("settings.lighting.god_rays.label")}
          description={t("settings.lighting.god_rays.description")}
          active={show_god_rays}
          on_toggle={toggle_god_rays}
          cost="high"
          dev
        />
      )}
      {__IS_DEV__ && (
        <SettingRow
          label={t("settings.lighting.shadow_map.label")}
          description={t("settings.lighting.shadow_map.description")}
          active={show_shadow_map}
          on_toggle={toggle_shadow_map}
          cost="high"
          dev
        />
      )}
    </div>
  );
}
