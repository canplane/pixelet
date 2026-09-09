// @pxl/web/app/settings/sections/camera_section.tsx

import { Move3d } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { SectionTitle, SettingRadioRow, SettingRow } from "@/app/settings/setting_row.tsx";

/** Camera/world-interaction controls — the tilt-gesture lock, and whether
 * the eraser tool is allowed to dig into real terrain (enable_terrain_carving,
 * off by default — camera/input.tsx's handle_tap and markers/hover_cursor.tsx
 * both gate their own "eraser hovering bare terrain" branch on this same
 * flag). Its own section (not folded into quality/lighting/post-fx) because
 * these are input-behavior toggles, not rendering-cost tradeoffs — none of
 * the "cost" framing the rest of the effects tab uses applies to them.
 *
 * ground_snap and show_structures live here too (moved from DebugSection)
 * even though they're dev-only (__IS_DEV__-gated, dev badge) — both are
 * camera/scene-composition toggles a developer reaches for while eyeballing
 * this exact section's own concerns (camera behavior, what's rendered
 * around it), not general debug tooling like the gizmo overlay or dummy
 * toasts that stayed in DebugSection. */
export function CameraSection() {
  const { t } = useTranslation();

  const lock_camera_tilt = useSettingsStore((state) => state.lock_camera_tilt);
  const toggle_lock_camera_tilt = useSettingsStore((state) => state.toggle_lock_camera_tilt);
  const show_edge_shade = useSettingsStore((state) => state.show_edge_shade);
  const toggle_edge_shade = useSettingsStore((state) => state.toggle_edge_shade);
  const show_decor = useSettingsStore((state) => state.show_decor);
  const toggle_show_decor = useSettingsStore((state) => state.toggle_show_decor);
  const camera_terrain = useSettingsStore((state) => state.camera_terrain);
  const set_camera_terrain = useSettingsStore((state) => state.set_camera_terrain);

  const ground_snap = useSettingsStore((state) => state.ground_snap);
  const toggle_ground_snap = useSettingsStore((state) => state.toggle_ground_snap);

  const show_structures = useSettingsStore((state) => state.show_structures);
  const toggle_structures = useSettingsStore((state) => state.toggle_structures);

  const enable_terrain_carving = useSettingsStore((state) => state.enable_terrain_carving);
  const toggle_terrain_carving = useSettingsStore((state) => state.toggle_terrain_carving);

  const show_poi = useSettingsStore((state) => state.show_poi);
  const toggle_show_poi = useSettingsStore((state) => state.toggle_show_poi);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Move3d}>{t("settings.camera.section_title")}</SectionTitle>
      <SettingRow
        label={t("settings.camera.lock_tilt.label")}
        description={t("settings.camera.lock_tilt.description")}
        active={lock_camera_tilt}
        on_toggle={toggle_lock_camera_tilt}
      />
      <SettingRow
        label={t("settings.camera.edge_shade.label")}
        description={t("settings.camera.edge_shade.description")}
        active={show_edge_shade}
        on_toggle={toggle_edge_shade}
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
      <SettingRow
        label={t("settings.camera.terrain_carving.label")}
        description={t("settings.camera.terrain_carving.description")}
        active={enable_terrain_carving}
        on_toggle={toggle_terrain_carving}
      />
      <SettingRow
        label={t("settings.camera.show_poi.label")}
        description={t("settings.camera.show_poi.description")}
        active={show_poi}
        on_toggle={toggle_show_poi}
      />
      <SettingRow
        label={t("settings.camera.show_decor.label")}
        description={t("settings.camera.show_decor.description")}
        active={show_decor}
        on_toggle={toggle_show_decor}
      />
      {__IS_DEV__ && (
        <SettingRow
          label={t("settings.debug.ground_snap.label")}
          description={t("settings.debug.ground_snap.description")}
          active={ground_snap}
          on_toggle={toggle_ground_snap}
          dev
        />
      )}
      {__IS_DEV__ && (
        <SettingRow
          label={t("settings.debug.structures.label")}
          description={t("settings.debug.structures.description")}
          active={show_structures}
          on_toggle={toggle_structures}
          dev
        />
      )}
    </div>
  );
}
