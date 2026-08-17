// @pxl/web/app/settings/sections/quality_section.tsx

import { Gauge } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useRenderSettingsStore } from "@/render/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

/** Base render-quality toggles — what gets drawn and at what resolution,
 * independent of the effect stack.
 *
 * Rows run cheapest first, as they do in lighting_section.tsx and
 * post_fx_section.tsx. That ordering is the point of the cost chips: the
 * settings legend tells you to switch off from the HIGH end, so the list has
 * to be readable from the bottom up. */
export function QualitySection() {
  const { t } = useTranslation();

  const low_spec = useRenderSettingsStore((state) => state.low_spec);
  const toggle_low_spec = useRenderSettingsStore((state) => state.toggle_low_spec);
  const show_structures = useRenderSettingsStore((state) => state.show_structures);
  const toggle_structures = useRenderSettingsStore((state) => state.toggle_structures);
  const high_dpr = useRenderSettingsStore((state) => state.high_dpr);
  const toggle_high_dpr = useRenderSettingsStore((state) => state.toggle_high_dpr);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Gauge} caption={t("settings.quality.section_caption")}>{t("settings.quality.section_title")}</SectionTitle>
      {
        /* cost="low": enabling this changes which material a cell resolves to
        (adapter.ts's ground_material_at simply stops skipping the channel) —
        the cell count and the per-cell work are identical either way. The
        part that is NOT measured is meshing: more material variety means
        more boundaries for the greedy mesher to break runs at. If buildings
        ever turn out to cost more than this chip claims, that is where to
        look. */
      }
      <SettingRow
        label={t("settings.quality.structures.label")}
        description={t("settings.quality.structures.description")}
        active={show_structures}
        on_toggle={toggle_structures}
        cost="low"
      />
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
