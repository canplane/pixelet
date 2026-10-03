// @pxl/web/feature/map/geocode/depth.ts

import type { f64, u8 } from "@xpute/core/abi/word.ts";
import { PLACE_RANK } from "./place.spec.ts";

/** Meters a 256-px Web Mercator tile's pixel spans at the equator at zoom 0. */
const M_PER_PX_AT_Z0 = (2 * Math.PI * 6378137) / 256;

/** Nominatim's zoom table read onto the place ladder: up to each zoom, the deepest rung named. */
const LADDER: readonly [f64, u8][] = [
  [4, PLACE_RANK.COUNTRY],
  [7, PLACE_RANK.REGION],
  [9, PLACE_RANK.COUNTY],
  [11, PLACE_RANK.CITY],
  [13, PLACE_RANK.DISTRICT],
  [15, PLACE_RANK.NEIGHBORHOOD],
  [17, PLACE_RANK.STREET],
];

/** The deepest rung a view at `mPerPx` names; standing at the center, or a NaN zoom, every rung. */
export function deepestRank(mPerPx: f64, lat: f64): u8 {
  if (!(mPerPx > 0)) return PLACE_RANK.BUILDING;
  const zoom = Math.log2((M_PER_PX_AT_Z0 * Math.cos((lat * Math.PI) / 180)) / mPerPx);
  return LADDER.find(([top]) => zoom <= top)?.[1] ?? PLACE_RANK.BUILDING;
}
