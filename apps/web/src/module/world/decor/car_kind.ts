// @pxl/web/module/world/decor/car_kind.ts

/**
 * A DecorKind for ambient cars on major roads, from kind.json — see
 * tree_kind.ts for why this file stays asset-free.
 */

import { type DecorEligibility, type DecorKindSeed, type DecorRules, eligibility_of, rules_of } from "./scatter.ts";
import { decor_kinds } from "./kind.ts";

const SPEC = decor_kinds.car;

export const CAR_KIND: DecorKindSeed = {
  tag: SPEC.tag,
  density: SPEC.density,
  variant_count: SPEC.variant_count,
  orientation: SPEC.orientation,
};

export const CAR_RULES: DecorRules = rules_of(SPEC.rules);
export const is_car_eligible: DecorEligibility = eligibility_of(CAR_RULES);
