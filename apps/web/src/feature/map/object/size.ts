// @pxl/web/feature/map/object/size.ts

import type { f64 } from "@xpute/core/abi/word.ts";
import { type PlaceableCube, type PlaceableCubeId, placeableCubeId } from "@/catalog/cell/mod.ts";

import { OBJECT_SCALE_EXPONENTS, OBJECT_SIZES, type ObjectScaleExponent } from "./size.spec.ts";

export { OBJECT_SCALE_EXPONENTS, OBJECT_SIZES, type ObjectScaleExponent };

/** The tier a fresh session places at: the coarsest, one full cell. */
export const DEFAULT_OBJECT_SIZE: f64 = OBJECT_SIZES[0];

export const DECOR_SIZE: f64 = OBJECT_SIZES[0];

const DEFAULT_ACTIVE_SCALES: readonly ObjectScaleExponent[] = OBJECT_SCALE_EXPONENTS;

/** Per-cube override of which of OBJECT_SIZES' tiers mode-bar.tsx's size
 * flyout offers — empty today (every cube, flat color or terrain-styled,
 * offers every tier), reserved for a future one that shouldn't (a huge
 * painted mural block, say). Keyed by the full PlaceableCubeId (not just
 * CubeColorId) since the hotbar's own MRU list (ui-store.ts's recentPicks)
 * can now surface a terrain-styled pick too.
 *
 * This is the shell arranging what exists, not what exists — so it stays
 * here, hand-written, while the ladder itself is generated from
 * spec/pxl/spec/bake/cell.json into this directory and into the kernel. */
const ACTIVE_SCALES: Partial<Record<PlaceableCubeId, readonly ObjectScaleExponent[]>> = {};

export function objectScaleExponents(cube: PlaceableCube): readonly ObjectScaleExponent[] {
  return ACTIVE_SCALES[placeableCubeId(cube)] ?? DEFAULT_ACTIVE_SCALES;
}
