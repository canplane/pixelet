// @pxl/web/feature/map/ui/overlay/poi-details.tsx

import { useTranslation } from "react-i18next";
import { kernel_action, useKernel } from "@/kernel/shell.ts";

export function PoiDetails() {
  const pages = useKernel((s) => s.poi);
  const { t } = useTranslation();
  if (!pages.length) return null;
  return (
    <details className="pointer-events-auto max-w-[240px] rounded-[6px] bg-[#FFFDF7] p-2 text-sm text-[#5B4636] shadow-sm">
      <summary className="cursor-pointer">{t("map.poi.places")}</summary>
      <div className="max-h-[35vh] overflow-y-auto">
        {pages.map((page) => (
          <section key={page.key} className="py-2">
            {page.count !== "1" && <p>{t("map.poi.count", { total: page.count })}</p>}
            {!page.records.length && <p>{t("map.poi.loading")}</p>}
            <ul>
              {page.records.map((record) => <li key={record.id} className="break-words py-1">{record.text || record.id}</li>)}
            </ul>
            <div className="flex gap-3">
              {page.offset !== "0" && <button type="button" className="min-h-8 underline" onClick={() => kernel_action({ type: "poi_page", key: page.key, next: false })}>{t("map.poi.first")}</button>}
              {page.next && <button type="button" className="min-h-8 underline" onClick={() => kernel_action({ type: "poi_page", key: page.key, next: true })}>{t("map.poi.next")}</button>}
            </div>
          </section>
        ))}
      </div>
    </details>
  );
}
