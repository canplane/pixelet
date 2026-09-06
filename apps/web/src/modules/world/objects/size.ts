// @pxl/web/modules/world/objects/size.ts

import type { f64 } from "@xpute/core/kit/word.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import type { CubeColorId } from "@pxl/shared/spec/cell/mod.ts";

/**
 * Placeable cube sizes — grid-aligned steps down from one terrain cell
 * (U_PER_CELL). Single source of truth for mode_bar's size selector, the
 * placement/erase raycasting and overlap check (camera/ray.ts,
 * objects/collision.ts), and the hover preview + real cube render
 * (world/markers/hover_cursor.tsx, layer/chunk_stage.tsx — every size,
 * including sub-cell, goes through the same dense voxel field there).
 * Tracks U_PER_CELL
 * itself, not a snapshot of its value, so a CELL_SHAMT change (constants.ts)
 * rescales every tier here automatically.
 *
 * Largest entry is U_PER_CELL because that's the terrain's own cell size —
 * an object at that size lines up 1:1 with a terrain cell (see
 * bake/constants.ts's OBJECT_LAYOUT, whose `max: CELL_SHAMT` is the other
 * half of that reasoning: objects/store.ts's sparse tree never spans more
 * than one cell) — the other three (finer) tiers are what render/layer/
 * objects/adapter.ts writes as sub-cell voxels (a per-voxel offset+size
 * payload within voxel.ts's dense field, see its own doc comment)
 * instead of a full-cell voxel. Deliberately 4 tiers deep, not 3 — when
 * CELL_SHAMT changed
 * U_PER_CELL 0.5u -> 1u, sliding a fixed *count* of tiers down from it would
 * have silently dropped the finest one (0.125u) that building at sub-block
 * detail already depended on, instead of just gaining a coarser option to
 * match the new cell size. One more halving step keeps every size that
 * already existed usable.
 */
export const OBJECT_SIZES: readonly f64[] = [U_PER_CELL, U_PER_CELL / 2, U_PER_CELL / 4, U_PER_CELL / 8];

export const DEFAULT_OBJECT_SIZE: f64 = OBJECT_SIZES[0];

/** Every decor kind places at exactly one terrain cell — unlike a color
 * cube, decor has no mode_bar size flyout of its own (yet) to vary this, and
 * a scattered tree (decor/tree_kind.ts) already assumes its own candidates
 * are one full cell wide. Deliberately the same value as OBJECT_SIZES[0],
 * not a re-export of it — object_size (ui_store.ts) is a live, user-changed
 * selection for the object tool specifically; this is decor's own fixed
 * constant and must keep working even if a future size flyout moves
 * object_size elsewhere. */
export const DECOR_SIZE: f64 = OBJECT_SIZES[0];

export const OBJECT_SCALE_EXPONENTS = [0, -1, -2, -3] as const;
export type ObjectScaleExponent = typeof OBJECT_SCALE_EXPONENTS[number];

const DEFAULT_ACTIVE_SCALES: readonly ObjectScaleExponent[] = [0, -1, -2, -3];

/** Per-color override of which of OBJECT_SIZES' tiers mode_bar.tsx's size
 * flyout offers — empty today (every color offers every tier), reserved for
 * a future color that shouldn't (a huge painted mural block, say). */
const ACTIVE_SCALES: Partial<Record<CubeColorId, readonly ObjectScaleExponent[]>> = {};

export function object_scale_exponents(id: CubeColorId): readonly ObjectScaleExponent[] {
  return ACTIVE_SCALES[id] ?? DEFAULT_ACTIVE_SCALES;
}
