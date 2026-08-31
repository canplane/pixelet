// @pxl/web/app/settings/sections/debug_section.tsx

import { useSyncExternalStore } from "react";
import { Bug } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useDebugStore } from "@/app/dev/debug_store.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { useRenderSettingsStore } from "@/render/settings_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";
import { Toast } from "@/shared/ui/toast/toast.tsx";
import { get_toast_entries, subscribe_toast_entries, toast_custom, toast_dismiss } from "@/shared/ui/toast/store.ts";

const DUMMY_TOAST_IDS = ["dummy-1", "dummy-2", "dummy-3", "dummy-4"];

/** Reads the toast store directly rather than local state: this section
 * unmounts whenever settings closes, which would reset a plain useState
 * to false even while the dummy toasts were still on screen. */
function any_dummy_toast_shown(): boolean {
  return get_toast_entries().some((entry) => DUMMY_TOAST_IDS.includes(entry.id));
}

function spawn_dummy_toasts(t: ReturnType<typeof useTranslation>["t"]) {
  DUMMY_TOAST_IDS.forEach((id, i) => {
    const message = t("settings.debug.dummy_toast.message", { index: i + 1 });
    toast_custom(id, ({ index, stack_size, is_removing }) => <Toast id={id} index={index} stack_size={stack_size} is_removing={is_removing} message={message} />);
  });
}

function dismiss_dummy_toasts() {
  DUMMY_TOAST_IDS.forEach((id) => toast_dismiss(id));
}

export function DebugSection() {
  const { t } = useTranslation();

  const low_spec = useRenderSettingsStore((state) => state.low_spec);
  const toggle_low_spec = useRenderSettingsStore((state) => state.toggle_low_spec);
  const show_structures = useRenderSettingsStore((state) => state.show_structures);
  const toggle_structures = useRenderSettingsStore((state) => state.toggle_structures);

  const show_debug_objects = useDebugStore((state) => state.show_debug_objects);
  const toggle_debug_objects = useDebugStore((state) => state.toggle_debug_objects);

  const ground_snap = useCameraStore((state) => state.ground_snap);
  const toggle_ground_snap = useCameraStore((state) => state.toggle_ground_snap);

  const dummy_toasts_shown = useSyncExternalStore(subscribe_toast_entries, any_dummy_toast_shown);
  function toggle_dummy_toasts() {
    if (dummy_toasts_shown) dismiss_dummy_toasts();
    else spawn_dummy_toasts(t);
  }

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Bug}>{t("settings.debug.section_title")}</SectionTitle>
      <SettingRow
        label={t("settings.debug.texture_pack.label")}
        description={t("settings.debug.texture_pack.description")}
        active={!low_spec}
        on_toggle={toggle_low_spec}
        cost="med"
      />
      <SettingRow
        label={t("settings.debug.structures.label")}
        description={t("settings.debug.structures.description")}
        active={show_structures}
        on_toggle={toggle_structures}
        cost="low"
      />
      <SettingRow
        label={t("settings.debug.objects.label")}
        description={t("settings.debug.objects.description")}
        active={show_debug_objects}
        on_toggle={toggle_debug_objects}
      />
      <SettingRow
        label={t("settings.debug.ground_snap.label")}
        description={t("settings.debug.ground_snap.description")}
        active={ground_snap}
        on_toggle={toggle_ground_snap}
      />
      <SettingRow
        label={t("settings.debug.dummy_toast.label")}
        description={t("settings.debug.dummy_toast.description")}
        active={dummy_toasts_shown}
        on_toggle={toggle_dummy_toasts}
      />
    </div>
  );
}
