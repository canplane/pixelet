// @pxl/web/app/settings/sections/post_fx_section.tsx

import { Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useRenderSettingsStore } from "@/render/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

/**
 * Screen-space post-processing toggles (scene.tsx's EffectComposer) — all
 * default off, real cost confirmed via profiling as a group. The `cost`
 * tags below split that group by typical per-technique cost — reasoned
 * from what each technique actually does (and, for AO, from how it's
 * actually configured here: post_cfg.AO's QUALITY: "performance" +
 * HALF_RES: true are both deliberate cost cuts off N8AO's own defaults,
 * not an out-of-the-box SSAO cost — so it lands at "med" alongside Bloom's
 * own downsampled mipmap blur, not "high"), not a separate individual
 * measurement per effect. Ordered low-to-high top-to-bottom.
 */
export function PostFxSection() {
  const { t } = useTranslation();

  const show_ao = useRenderSettingsStore((state) => state.show_ao);
  const toggle_ao = useRenderSettingsStore((state) => state.toggle_ao);
  const show_bloom = useRenderSettingsStore((state) => state.show_bloom);
  const toggle_bloom = useRenderSettingsStore((state) => state.toggle_bloom);
  const show_tilt_shift = useRenderSettingsStore((state) => state.show_tilt_shift);
  const toggle_tilt_shift = useRenderSettingsStore((state) => state.toggle_tilt_shift);
  const show_chromatic_aberration = useRenderSettingsStore((state) => state.show_chromatic_aberration);
  const toggle_chromatic_aberration = useRenderSettingsStore((state) => state.toggle_chromatic_aberration);
  const show_smaa = useRenderSettingsStore((state) => state.show_smaa);
  const toggle_smaa = useRenderSettingsStore((state) => state.toggle_smaa);

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
        label={t("settings.post_fx.smaa.label")}
        description={t("settings.post_fx.smaa.description")}
        active={show_smaa}
        on_toggle={toggle_smaa}
        cost="low"
      />
      <SettingRow
        label={t("settings.post_fx.tilt_shift.label")}
        description={t("settings.post_fx.tilt_shift.description")}
        active={show_tilt_shift}
        on_toggle={toggle_tilt_shift}
        cost="low"
      />
      <SettingRow
        label={t("settings.post_fx.bloom.label")}
        description={t("settings.post_fx.bloom.description")}
        active={show_bloom}
        on_toggle={toggle_bloom}
        cost="med"
      />
      <SettingRow
        label={t("settings.post_fx.ao.label")}
        description={t("settings.post_fx.ao.description")}
        active={show_ao}
        on_toggle={toggle_ao}
        cost="med"
      />
    </div>
  );
}
