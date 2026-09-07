// @pxl/web/app/settings/sections/camera_section.tsx

import { Move3d } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

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

  const ground_snap = useSettingsStore((state) => state.ground_snap);
  const toggle_ground_snap = useSettingsStore((state) => state.toggle_ground_snap);

  const show_structures = useSettingsStore((state) => state.show_structures);
  const toggle_structures = useSettingsStore((state) => state.toggle_structures);

  const enable_terrain_carving = useSettingsStore((state) => state.enable_terrain_carving);
  const toggle_terrain_carving = useSettingsStore((state) => state.toggle_terrain_carving);

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
        label={t("settings.camera.terrain_carving.label")}
        description={t("settings.camera.terrain_carving.description")}
        active={enable_terrain_carving}
        on_toggle={toggle_terrain_carving}
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
