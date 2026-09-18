// @pxl/web/catalog/object/size.ts

import type { f64 } from "@xpute/core/kit/word.ts";
import { placeable_cube_id, type PlaceableCube, type PlaceableCubeId } from "@/catalog/cell/mod.ts";

import { OBJECT_SCALE_EXPONENTS, OBJECT_SIZES, type ObjectScaleExponent } from "./size_spec.ts";

export { OBJECT_SCALE_EXPONENTS, OBJECT_SIZES, type ObjectScaleExponent };

/** The tier a fresh session places at: the coarsest, one full cell. */
export const DEFAULT_OBJECT_SIZE: f64 = OBJECT_SIZES[0];

export const DECOR_SIZE: f64 = OBJECT_SIZES[0];

const DEFAULT_ACTIVE_SCALES: readonly ObjectScaleExponent[] = OBJECT_SCALE_EXPONENTS;

/** Per-cube override of which of OBJECT_SIZES' tiers mode_bar.tsx's size
 * flyout offers — empty today (every cube, flat color or terrain-styled,
 * offers every tier), reserved for a future one that shouldn't (a huge
 * painted mural block, say). Keyed by the full PlaceableCubeId (not just
 * CubeColorId) since the hotbar's own MRU list (ui_store.ts's recent_picks)
 * can now surface a terrain-styled pick too.
 *
 * This is the shell arranging what exists, not what exists — so it stays
 * here, hand-written, while the ladder itself is generated from
 * spec/bake/cell.json into this directory and into the kernel. */
const ACTIVE_SCALES: Partial<Record<PlaceableCubeId, readonly ObjectScaleExponent[]>> = {};

export function object_scale_exponents(cube: PlaceableCube): readonly ObjectScaleExponent[] {
  return ACTIVE_SCALES[placeable_cube_id(cube)] ?? DEFAULT_ACTIVE_SCALES;
}
