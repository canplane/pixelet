// @pxl/web/module/world/decor/boat_kind.ts

/**
 * A DecorKind for small boats on open water, from kind.json — see
 * tree_kind.ts for why this file stays asset-free.
 */

import { type DecorEligibility, type DecorKindSeed, type DecorRules, eligibility_of, rules_of } from "./scatter.ts";
import { decor_kinds } from "./kind.ts";

const SPEC = decor_kinds.boat;

export const BOAT_KIND: DecorKindSeed = {
  tag: SPEC.tag,
  density: SPEC.density,
  variant_count: SPEC.variant_count,
  orientation: SPEC.orientation,
};

export const BOAT_RULES: DecorRules = rules_of(SPEC.rules);
export const is_boat_eligible: DecorEligibility = eligibility_of(BOAT_RULES);
