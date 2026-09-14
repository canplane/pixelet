// @pxl/web/module/world/decor/train_kind.ts

/**
 * A DecorKind for occasional trains on rail lines, from kind.json — see
 * tree_kind.ts for why this file stays asset-free.
 */

import { type DecorEligibility, type DecorKindSeed, type DecorRules, eligibility_of, rules_of } from "./scatter.ts";
import { decor_kinds } from "./kind.ts";

const SPEC = decor_kinds.train;

export const TRAIN_KIND: DecorKindSeed = {
  tag: SPEC.tag,
  density: SPEC.density,
  variant_count: SPEC.variant_count,
  orientation: SPEC.orientation,
};

export const TRAIN_RULES: DecorRules = rules_of(SPEC.rules);
export const is_train_eligible: DecorEligibility = eligibility_of(TRAIN_RULES);
