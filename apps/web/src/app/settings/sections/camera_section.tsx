// @pxl/web/app/settings/sections/camera_section.tsx

import { Move3d } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useDebugStore } from "@/app/dev/debug_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";

/** Camera interaction controls — currently just the tilt-gesture lock.
 * Its own section (not folded into quality/lighting/post-fx) because it's
 * an input-behavior toggle, not a rendering-cost tradeoff — none of the
 * "cost" framing the rest of the effects tab uses applies to it. */
export function CameraSection() {
  const { t } = useTranslation();

  const lock_camera_tilt = useDebugStore((state) => state.lock_camera_tilt);
  const toggle_lock_camera_tilt = useDebugStore((state) => state.toggle_lock_camera_tilt);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Move3d}>{t("settings.camera.section_title")}</SectionTitle>
      <SettingRow
        label={t("settings.camera.lock_tilt.label")}
        description={t("settings.camera.lock_tilt.description")}
        active={lock_camera_tilt}
        on_toggle={toggle_lock_camera_tilt}
      />
    </div>
  );
}
