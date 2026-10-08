// @pxl/web/feature/world/division/center.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { useTranslation } from "react-i18next";

import type { DivisionPart, LonLat } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { DIVISION_SUBTYPE } from "./division.spec.ts";
import { EARTH_RADIUS_M } from "@/config/earth.spec.ts";
import { assembleLabel, primaryLanguage } from "./label.ts";

/** The focus's division, assembled per snapshot, so a language switch needs no refetch. */
export function useDivisionLabel(): string | null {
  const { i18n } = useTranslation();
  return useKernel((s) => {
    const zoom = webZoom(s.m_per_px, s.focus.center?.lat ?? 0);
    const parts = s.division.filter((p) => shownFrom(p) <= zoom);
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

/** The web map's zoom the view is at; every name shows where the meters a pixel spans are unknown. */
function webZoom(mPerPx: f64, lat: f64): f64 {
  return mPerPx > 0 ? Math.log2((M_PER_PX_AT_Z0 * Math.cos((lat * Math.PI) / 180)) / mPerPx) : Infinity;
}

/** The zoom a division's name shows from, as Overture's own tiles place it
 * (overture-tiles profiles/Divisions.java): by its prominence where it has
 * one, else by its subtype. */
function shownFrom(p: DivisionPart): f64 {
  if (p.prominence > 0) return Math.min(Math.floor((100 - p.prominence) / 5) + 1, 10);
  switch (p.subtype) {
    case DIVISION_SUBTYPE.COUNTRY:
    case DIVISION_SUBTYPE.DEPENDENCY:
      return 0;
    case DIVISION_SUBTYPE.REGION:
      return 4;
    case DIVISION_SUBTYPE.COUNTY:
      return 8;
    default:
      return 10;
  }
}
