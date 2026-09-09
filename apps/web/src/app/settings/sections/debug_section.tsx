// @pxl/web/app/settings/sections/debug_section.tsx

import { useSyncExternalStore } from "react";
import { Bug } from "lucide-react";
import { useTranslation } from "react-i18next";

import { useDebugStore } from "@/app/settings/debug_store.ts";
import { SectionTitle, SettingRow } from "@/app/settings/setting_row.tsx";
import { Toast } from "@/shared/ui/toast/toast.tsx";
import { getToastEntries, subscribeToastEntries, toastCustom, toastDismiss } from "@/shared/ui/toast/store.ts";

const DUMMY_TOAST_IDS = ["dummy-1", "dummy-2", "dummy-3", "dummy-4"];

/** Reads the toast store directly rather than local state: this section
 * unmounts whenever settings closes, which would reset a plain useState
 * to false even while the dummy toasts were still on screen. */
function any_dummy_toast_shown(): boolean {
  return getToastEntries().some((entry) => DUMMY_TOAST_IDS.includes(entry.id));
}

function spawn_dummy_toasts(t: ReturnType<typeof useTranslation>["t"]) {
  DUMMY_TOAST_IDS.forEach((id, i) => {
    const message = t("settings.debug.dummy_toast.message", { index: i + 1 });
    toastCustom(
      id,
      ({ index, stackSize, isRemoving }) => <Toast id={id} index={index} stackSize={stackSize} isRemoving={isRemoving} message={message} className="border-transparent" />,
    );
  });
}

function dismiss_dummy_toasts() {
  DUMMY_TOAST_IDS.forEach((id) => toastDismiss(id));
}

export function DebugSection() {
  const { t } = useTranslation();

  const showDebugObjects = useDebugStore((state) => state.showDebugObjects);
  const toggleDebugObjects = useDebugStore((state) => state.toggleDebugObjects);

  const showLedgerOverlay = useDebugStore((state) => state.showLedgerOverlay);
  const toggleLedgerOverlay = useDebugStore((state) => state.toggleLedgerOverlay);

  const logLedger = useDebugStore((state) => state.logLedger);
  const toggleLogLedger = useDebugStore((state) => state.toggleLogLedger);

  const dummy_toasts_shown = useSyncExternalStore(subscribeToastEntries, any_dummy_toast_shown);
  function toggle_dummy_toasts() {
    if (dummy_toasts_shown) dismiss_dummy_toasts();
    else spawn_dummy_toasts(t);
  }

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={Bug} dev>{t("settings.debug.section_title")}</SectionTitle>
      <SettingRow
        label={t("settings.debug.objects.label")}
        description={t("settings.debug.objects.description")}
        active={showDebugObjects}
        on_toggle={toggleDebugObjects}
      />
      <SettingRow
        label={t("settings.debug.ledger_overlay.label")}
        description={t("settings.debug.ledger_overlay.description")}
        active={showLedgerOverlay}
        on_toggle={toggleLedgerOverlay}
      />
      <SettingRow
        label={t("settings.debug.ledger_log.label")}
        description={t("settings.debug.ledger_log.description")}
        active={logLedger}
        on_toggle={toggleLogLedger}
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
