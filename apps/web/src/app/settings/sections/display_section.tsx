// @pxl/web/app/settings/sections/display_section.tsx

import { Eye } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

/** What the world shows. Turning one of these off removes information —
 * a badge, a scattered tree, a building — which is what separates them
 * from the fidelity dials in QualitySection and the readability aids in
 * LightingSection: those leave the same world, drawn differently. */
export function DisplaySection() {
  const { t } = useTranslation();

  const show_poi = useSettingsStore((state) => state.show_poi);
  const toggle_show_poi = useSettingsStore((state) => state.toggle_show_poi);
  const show_decor = useSettingsStore((state) => state.show_decor);
  const toggle_show_decor = useSettingsStore((state) => state.toggle_show_decor);
  const show_structures = useSettingsStore((state) => state.show_structures);
  const toggle_structures = useSettingsStore((state) => state.toggle_structures);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Eye} caption={t("settings.display.section_caption")}>
        {t("settings.display.section_title")}
      </SectionTitle>
      <SettingRow
        label={t("settings.display.show_poi.label")}
        description={t("settings.display.show_poi.description")}
        active={show_poi}
        on_toggle={toggle_show_poi}
      />
      <SettingRow
        label={t("settings.display.show_decor.label")}
        description={t("settings.display.show_decor.description")}
        active={show_decor}
        on_toggle={toggle_show_decor}
        cost="low"
      />
      {__IS_DEV__ && (
        <SettingRow
          label={t("settings.display.show_structures.label")}
          description={t("settings.display.show_structures.description")}
          active={show_structures}
          on_toggle={toggle_structures}
          dev
        />
      )}
    </div>
  );
}
