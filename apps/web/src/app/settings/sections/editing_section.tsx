// @pxl/web/app/settings/sections/editing_section.tsx

import { Pickaxe } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

/** What the tools are allowed to change. Terrain carving is the only one
 * so far: camera/input.tsx's handle_tap and markers/hover_cursor.tsx both
 * gate their "eraser over bare terrain" branch on it. */
export function EditingSection() {
  const { t } = useTranslation();

  const enable_terrain_carving = useSettingsStore((state) => state.enable_terrain_carving);
  const toggle_terrain_carving = useSettingsStore((state) => state.toggle_terrain_carving);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Pickaxe} caption={t("settings.editing.section_caption")}>
        {t("settings.editing.section_title")}
      </SectionTitle>
      <SettingRow
        label={t("settings.editing.terrain_carving.label")}
        description={t("settings.editing.terrain_carving.description")}
        active={enable_terrain_carving}
        on_toggle={toggle_terrain_carving}
      />
    </div>
  );
}
