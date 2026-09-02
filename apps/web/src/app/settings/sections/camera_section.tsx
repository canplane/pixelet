// @pxl/web/app/settings/sections/camera_section.tsx

import { Move3d } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

/** Camera interaction controls — currently just the tilt-gesture lock.
 * Its own section (not folded into quality/lighting/post-fx) because it's
 * an input-behavior toggle, not a rendering-cost tradeoff — none of the
 * "cost" framing the rest of the effects tab uses applies to it.
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

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Move3d}>{t("settings.camera.section_title")}</SectionTitle>
      <SettingRow
        label={t("settings.camera.lock_tilt.label")}
        description={t("settings.camera.lock_tilt.description")}
        active={lock_camera_tilt}
        on_toggle={toggle_lock_camera_tilt}
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
