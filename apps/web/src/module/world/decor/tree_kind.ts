// @pxl/web/module/world/decor/tree_kind.ts

/**
 * The first concrete DecorKind — trees on green/forest terrain — from
 * kind.json, where its seed and rules are said and why. Deliberately
 * asset-free (no .glb/.png imports) — see tree_decor.ts for why those live
 * in a separate file instead of here.
 */

import { type DecorEligibility, type DecorKindSeed, type DecorRules, eligibility_of, rules_of } from "./scatter.ts";
import { decor_kinds } from "./kind.ts";

const SPEC = decor_kinds.tree;

export const TREE_KIND: DecorKindSeed = {
  tag: SPEC.tag,
  density: SPEC.density,
  variant_count: SPEC.variant_count,
  orientation: SPEC.orientation,
};

export const TREE_RULES: DecorRules = rules_of(SPEC.rules);
export const is_tree_eligible: DecorEligibility = eligibility_of(TREE_RULES);
