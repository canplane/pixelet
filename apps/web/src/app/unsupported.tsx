// @pxl/web/app/unsupported.tsx

import { useTranslation } from "react-i18next";

import { useUnsupportedStore } from "./store/unsupported-store.ts";

/**
 * The end of the session, when the browser turns out not to be able to run
 * this. Opaque and over everything: what is behind it is a map that will never
 * finish loading, and leaving it visible under a message would read as a page
 * still working.
 */
export function UnsupportedNotice() {
  const { t } = useTranslation();
  const reason = useUnsupportedStore((state) => state.reason);
  if (reason === null) return null;
  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center gap-3 bg-background px-8 text-center">
      <h1 className="text-lg font-semibold text-foreground">{t("unsupported.title")}</h1>
      <p className="max-w-sm text-sm leading-relaxed text-muted-foreground">{t(`unsupported.${reason}`)}</p>
    </div>
  );
}
