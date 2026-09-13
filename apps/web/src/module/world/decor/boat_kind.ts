// @pxl/web/module/world/decor/boat_kind.ts

/**
 * A DecorKind's placement rules for small boats on open water — see
 * tree_kind.ts's own doc comment for why this file stays asset-free.
 */

import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { type DecorEligibility, type DecorKindSeed, type DecorRules, eligibility_of } from "./scatter.ts";

const BOAT_KIND_TAG = 0xb0a7_5eed_0002_dec0n;

// Sparser than trees' own 0.02 would look right here — most rivers are only
// a few cells wide, so the same per-cell rate reads far denser on a narrow
// water strip than it does scattered across a wide green area. Cut to 1/4
// of the first-pass value (0.015) after that read as boats covering the
// water, not scattered on it.
const BOAT_DENSITY = 0.002;

// The palette's model rows for "boat"; boat_decor.ts asserts the two agree.
const BOAT_VARIANT_COUNT = 4;

export const BOAT_KIND: DecorKindSeed = {
  tag: BOAT_KIND_TAG,
  density: BOAT_DENSITY,
  variant_count: BOAT_VARIANT_COUNT,
  orientation: "random",
};

/** Open river/lake water only — WETLAND is excluded (too shallow/marshy for
 * a boat to sit in), and structure/transport must both still be VOID: per
 * render/layer/terrain/adapter.ts's own ground_material_at precedence,
 * either one (a pier, a bridge deck) would win over water and actually
 * render as itself, not water, at that cell — a boat there would visually
 * float on top of a bridge instead of the water beneath it. */
export const BOAT_RULES: DecorRules = {
  land: "any",
  water: [cube_types.water.RIVER, cube_types.water.LAKE],
  structure: "void",
  transport: "void",
};
export const is_boat_eligible: DecorEligibility = eligibility_of(BOAT_RULES);
