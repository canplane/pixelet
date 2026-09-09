// @pxl/web/app/settings/sections/camera_section.tsx

import { Move3d } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { SectionTitle, SettingRadioRow, SettingRow } from "@/app/settings/setting_row.tsx";

/** How the view moves: the three orbit controls, and what terrain is
 * allowed to do to them. Nothing here changes what is drawn, so the cost
 * framing the rest of the effects tab uses does not apply. */
export function CameraSection() {
  const { t } = useTranslation();

  const lock_camera_tilt = useSettingsStore((state) => state.lock_camera_tilt);
  const toggle_lock_camera_tilt = useSettingsStore((state) => state.toggle_lock_camera_tilt);
  const camera_terrain = useSettingsStore((state) => state.camera_terrain);
  const set_camera_terrain = useSettingsStore((state) => state.set_camera_terrain);
  const ground_snap = useSettingsStore((state) => state.ground_snap);
  const toggle_ground_snap = useSettingsStore((state) => state.toggle_ground_snap);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Move3d} caption={t("settings.camera.section_caption")}>
        {t("settings.camera.section_title")}
      </SectionTitle>
      <SettingRow
        label={t("settings.camera.lock_tilt.label")}
        description={t("settings.camera.lock_tilt.description")}
        active={lock_camera_tilt}
        on_toggle={toggle_lock_camera_tilt}
      />
      {
        /* Grouped as radios rather than switches: one answer to "what does
          terrain do to the camera", not three independent behaviours. */
      }
      <div className="flex flex-col gap-0.5 -mx-3 rounded-[10px] border border-border p-1">
        <SettingRadioRow
          label={t("settings.camera.terrain.through.label")}
          description={t("settings.camera.terrain.through.description")}
          selected={camera_terrain === "through"}
          on_select={() => set_camera_terrain("through")}
        />
        <SettingRadioRow
          label={t("settings.camera.terrain.stop.label")}
          description={t("settings.camera.terrain.stop.description")}
          selected={camera_terrain === "stop"}
          on_select={() => set_camera_terrain("stop")}
        />
        <SettingRadioRow
          label={t("settings.camera.terrain.climb.label")}
          description={t("settings.camera.terrain.climb.description")}
          selected={camera_terrain === "climb"}
          on_select={() => set_camera_terrain("climb")}
        />
      </div>
      {__IS_DEV__ && (
        <SettingRow
          label={t("settings.camera.ground_snap.label")}
          description={t("settings.camera.ground_snap.description")}
          active={ground_snap}
          on_toggle={toggle_ground_snap}
          dev
        />
      )}
    </div>
  );
}
