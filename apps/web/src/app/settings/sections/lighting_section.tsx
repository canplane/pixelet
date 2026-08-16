// @pxl/web/app/settings/sections/lighting_section.tsx

import { Sun } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useRenderSettingsStore } from "@/render/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

/**
 * Real-time lighting/shadow toggles — environment.tsx's sun/moon lights and
 * the effects layered on top of them, ordered low-to-high cost top-to-
 * bottom. Both god_rays and shadow_map are tagged "high" (only 3 tiers
 * exist), but shadow_map's own doc comment (render/settings_store.ts)
 * explicitly calls it out as the single most expensive toggle in the whole
 * store — a full extra shadow-camera depth pass per light — while
 * god_rays is only "likely pricier" than the med-tier post-stack effects,
 * a weaker claim. shadow_map goes last.
 */
export function LightingSection() {
  const { t } = useTranslation();

  const show_shadow_map = useRenderSettingsStore((state) => state.show_shadow_map);
  const toggle_shadow_map = useRenderSettingsStore((state) => state.toggle_shadow_map);
  const show_blob_shadow = useRenderSettingsStore((state) => state.show_blob_shadow);
  const toggle_blob_shadow = useRenderSettingsStore((state) => state.toggle_blob_shadow);
  const show_rim_light = useRenderSettingsStore((state) => state.show_rim_light);
  const toggle_rim_light = useRenderSettingsStore((state) => state.toggle_rim_light);
  const show_god_rays = useRenderSettingsStore((state) => state.show_god_rays);
  const toggle_god_rays = useRenderSettingsStore((state) => state.toggle_god_rays);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Sun} caption={t("settings.lighting.section_caption")}>
        {t("settings.lighting.section_title")}
      </SectionTitle>
      <SettingRow
        label={t("settings.lighting.blob_shadow.label")}
        description={t("settings.lighting.blob_shadow.description")}
        active={show_blob_shadow}
        on_toggle={toggle_blob_shadow}
        cost="low"
      />
      <SettingRow
        label={t("settings.lighting.rim_light.label")}
        description={t("settings.lighting.rim_light.description")}
        active={show_rim_light}
        on_toggle={toggle_rim_light}
        cost="low"
      />
      <SettingRow
        label={t("settings.lighting.god_rays.label")}
        description={t("settings.lighting.god_rays.description")}
        active={show_god_rays}
        on_toggle={toggle_god_rays}
        cost="high"
      />
      <SettingRow
        label={t("settings.lighting.shadow_map.label")}
        description={t("settings.lighting.shadow_map.description")}
        active={show_shadow_map}
        on_toggle={toggle_shadow_map}
        cost="high"
      />
    </div>
  );
}
