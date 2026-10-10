// @pxl/web/feature/world/geocode/center.ts

import type { f64, u8 } from "@xpute/core/abi/word.ts";
import { useTranslation } from "react-i18next";

import type { LonLat } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { PLACE_RANK } from "./place.spec.ts";
import { EARTH_RADIUS_M } from "@/config/earth.spec.ts";
import { assembleLabel, primaryLanguage } from "./label.ts";

/** The focus's place, assembled per snapshot, so a language switch needs no refetch. */
export function usePlaceLabel(): string | null {
  const { i18n } = useTranslation();
  return useKernel((s) => {
    const deepest = deepestRank(s.m_per_px, s.focus.center?.lat ?? 0);
    const parts = s.place.filter((p) => p.rank <= deepest);
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

/** Nominatim's zoom table. */
const LADDER: readonly [f64, u8][] = [
  [4, PLACE_RANK.COUNTRY],
  [7, PLACE_RANK.REGION],
  [9, PLACE_RANK.COUNTY],
  [11, PLACE_RANK.CITY],
  [13, PLACE_RANK.DISTRICT],
  [15, PLACE_RANK.NEIGHBORHOOD],
  [17, PLACE_RANK.STREET],
];

function deepestRank(mPerPx: f64, lat: f64): u8 {
  if (!(mPerPx > 0)) return PLACE_RANK.BUILDING;
  const zoom = Math.log2((M_PER_PX_AT_Z0 * Math.cos((lat * Math.PI) / 180)) / mPerPx);
  return LADDER.find(([top]) => zoom <= top)?.[1] ?? PLACE_RANK.BUILDING;
}
