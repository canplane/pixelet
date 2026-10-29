// @pxl/client/shell/division/focus.ts

import { useTranslation } from "react-i18next";

import type { LonLat } from "@/abi/cmd.spec.ts";
import { assembleLabel, primaryLanguage } from "./label.ts";
import { fixed } from "@/shell/fixed.ts";
import { useKernel } from "@/shell/store/watch.ts";

/** The focus's division, assembled per snapshot, so a language switch needs no refetch; the
 * kernel says which of its names the view is in far enough to show. */
export function useDivisionLabel(): string | null {
  const { i18n } = useTranslation();
  return useKernel((s) => {
    const parts = s.division.filter((p) => p.shown);
    return parts.length === 0
      ? null
      : assembleLabel(
          parts.map((p) => ({
            name: p.name,
            names: Object.fromEntries(p.names.map((n) => [n.lang, n.name])),
            subtype: p.subtype,
            area: p.area,
          })),
          primaryLanguage(i18n.language),
        );
  });
}

export function useCenterCoordinates(): string | null {
  return useKernel((s) => s.focus.center && coordinateLabel(s.focus.center));
}

/** Four decimals: about eleven meters. */
function coordinateLabel({ lat, lon }: LonLat): string {
  return `${fixed(Math.abs(lat), 4)}° ${lat < 0 ? "S" : "N"}, ${fixed(
    Math.abs(lon),
    4,
  )}° ${lon < 0 ? "W" : "E"}`;
}
