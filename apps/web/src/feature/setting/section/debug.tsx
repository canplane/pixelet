// @pxl/web/feature/setting/section/debug.tsx

import { useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";

import { SettingRow } from "@/feature/setting/row.tsx";
import { SettingsSection } from "@/feature/setting/section.tsx";
import { Toast } from "@/shared/ui/toast/toast.tsx";
import { getToastEntries, subscribeToastEntries, toastCustom, toastDismiss } from "@/shared/ui/toast/store.ts";

const DUMMY_TOAST_IDS = ["dummy-1", "dummy-2", "dummy-3", "dummy-4"];

/** Reads the toast store directly rather than local state: this section
 * unmounts whenever settings closes, which would reset a plain useState
 * to false even while the dummy toasts were still on screen. */
function anyDummyToastShown(): boolean {
  return getToastEntries().some((entry) => DUMMY_TOAST_IDS.includes(entry.id));
}

function spawnDummyToasts(t: ReturnType<typeof useTranslation>["t"]) {
  DUMMY_TOAST_IDS.forEach((id, i) => {
    const message = t("settings.debug.dummy_toast.message", { index: i + 1 });
    toastCustom(
      id,
      ({ index, stackSize, isRemoving }) => <Toast id={id} index={index} stackSize={stackSize} isRemoving={isRemoving} message={message} className="border-transparent" />,
    );
  });
}

function dismissDummyToasts() {
  DUMMY_TOAST_IDS.forEach((id) => toastDismiss(id));
}

export function DebugSection() {
  const { t } = useTranslation();

  const dummyToastsShown = useSyncExternalStore(subscribeToastEntries, anyDummyToastShown);
  function toggleDummyToasts() {
    if (dummyToastsShown) dismissDummyToasts();
    else spawnDummyToasts(t);
  }

  // The schema's debug toggles, then the one row that is not a setting:
  // the dummy toasts are transient UI state, spawned and dismissed, never
  // stored.
  return (
    <SettingsSection section="debug">
      <SettingRow
        label={t("settings.debug.dummy_toast.label")}
        description={t("settings.debug.dummy_toast.description")}
        active={dummyToastsShown}
        onToggle={toggleDummyToasts}
      />
    </SettingsSection>
  );
}
