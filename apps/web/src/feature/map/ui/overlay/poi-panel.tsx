// @pxl/web/feature/map/ui/overlay/poi-panel.tsx

import { useTranslation } from "react-i18next";

import { kernel_action, useKernel } from "@/kernel/shell.ts";

/** The badges on the map as a list, nearest what the view looks at first: a
 * place by its name, a crowd by how many it is. Choosing one goes to it a
 * level closer, where a crowd comes apart into the places under it — what
 * the list pages through is the map's own levels. */
export function PoiPanel() {
  const badges = useKernel((s) => s.poi);
  const { t } = useTranslation();
  if (!badges.length) return null;
  return (
    <details className="pointer-events-auto max-w-[240px] rounded-[6px] bg-[#FFFDF7] p-2 text-sm text-[#5B4636] shadow-sm">
      <summary className="cursor-pointer">{t("map.poi.places")}</summary>
      <ul className="max-h-[35vh] overflow-y-auto">
        {badges.map((b) => (
          <li key={b.id}>
            <button
              type="button"
              className="min-h-8 w-full break-words text-left"
              onClick={() => kernel_action({ type: "poi_focus", id: b.id })}
            >
              {b.name ?? t("map.poi.count", { count: b.count })}
            </button>
          </li>
        ))}
      </ul>
    </details>
  );
}
