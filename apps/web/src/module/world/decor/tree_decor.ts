// @pxl/web/module/world/decor/tree_decor.ts

/**
 * The tree kind as a bundle: tree_kind.ts's placement rules bound to the
 * palette's models for it, which is where the models, their skins and
 * their extents come from (packages/pxl/shared/src/record/shipped.ts).
 */

import { bind_palette, type DecorKindBundle } from "./decor_kinds.ts";
import { is_tree_eligible, TREE_KIND, TREE_RULES } from "./tree_kind.ts";

export const TREE_DECOR_BUNDLE: DecorKindBundle = bind_palette({
  kind: "tree",
  seed: TREE_KIND,
  rules: TREE_RULES,
  is_eligible: is_tree_eligible,
  decor_id: "tree",
});
