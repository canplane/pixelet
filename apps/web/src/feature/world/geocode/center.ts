// @pxl/web/feature/world/geocode/center.ts

import type { f64, u8 } from "@xpute/core/abi/word.ts";
import { useTranslation } from "react-i18next";

import type { LonLat } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { DIVISION_SUBTYPE } from "./division.spec.ts";
import { EARTH_RADIUS_M } from "@/config/earth.spec.ts";
import { assembleLabel, primaryLanguage } from "./label.ts";

/** The focus's division, assembled per snapshot, so a language switch needs no refetch. */
export function useDivisionLabel(): string | null {
  const { i18n } = useTranslation();
  return useKernel((s) => {
    const deepest = deepestSubtype(s.m_per_px, s.focus.center?.lat ?? 0);
    const parts = s.division.filter((p) => p.subtype <= deepest);
    return parts.length === 0 ? null : assembleLabel(parts.map((p) => ({ name: p.name, names: Object.fromEntries(p.names.map((n) => [n.lang, n.name])) })), primaryLanguage(i18n.language));
  });
}

export function useCenterCoordinates(): string | null {
  return useKernel((s) => s.focus.center && coordinateLabel(s.focus.center));
}

/** Four decimals: about eleven meters. */
function coordinateLabel({ lat, lon }: LonLat): string {
  return `${Math.abs(lat).toFixed(4)}° ${lat < 0 ? "S" : "N"}, ${Math.abs(lon).toFixed(4)}° ${lon < 0 ? "W" : "E"}`;
}

/** Web Mercator, 256-px tiles, equator, z0. */
const M_PER_PX_AT_Z0 = (2 * Math.PI * EARTH_RADIUS_M) / 256;

/** Nominatim's zoom table, its city a locality and its district a macrohood. */
const LADDER: readonly [f64, u8][] = [
  [4, DIVISION_SUBTYPE.COUNTRY],
  [7, DIVISION_SUBTYPE.REGION],
  [9, DIVISION_SUBTYPE.COUNTY],
  [11, DIVISION_SUBTYPE.LOCALITY],
  [13, DIVISION_SUBTYPE.MACROHOOD],
  [15, DIVISION_SUBTYPE.NEIGHBORHOOD],
];

function deepestSubtype(mPerPx: f64, lat: f64): u8 {
  if (!(mPerPx > 0)) return DIVISION_SUBTYPE.MICROHOOD;
  const zoom = Math.log2((M_PER_PX_AT_Z0 * Math.cos((lat * Math.PI) / 180)) / mPerPx);
  return LADDER.find(([top]) => zoom <= top)?.[1] ?? DIVISION_SUBTYPE.MICROHOOD;
}
