// @pxl/web/feature/map/geocode/center.ts

import { useTranslation } from "react-i18next";

import type { LonLat } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { deepestRank } from "./depth.ts";
import { assembleLabel, primaryLanguage } from "./label.ts";

/** The map center's place, assembled per snapshot, so a language switch needs no refetch. */
export function usePlaceLabel(): string | null {
  const { i18n } = useTranslation();
  return useKernel((s) => {
    const deepest = deepestRank(s.m_per_px, s.map.center?.lat ?? 0);
    const parts = s.place.filter((p) => p.rank <= deepest);
    return parts.length === 0 ? null : assembleLabel(parts.map((p) => ({ name: p.name, names: Object.fromEntries(p.names.map((n) => [n.lang, n.name])) })), primaryLanguage(i18n.language));
  });
}

export function useCenterCoordinates(): string | null {
  return useKernel((s) => s.map.center && coordinateLabel(s.map.center));
}

/** Four decimals: about eleven meters. */
function coordinateLabel({ lat, lon }: LonLat): string {
  return `${Math.abs(lat).toFixed(4)}° ${lat < 0 ? "S" : "N"}, ${Math.abs(lon).toFixed(4)}° ${lon < 0 ? "W" : "E"}`;
}
