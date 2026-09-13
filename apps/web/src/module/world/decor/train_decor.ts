// @pxl/web/module/world/decor/train_decor.ts

/**
 * The train kind as a bundle: train_kind.ts's placement rules bound to the
 * palette's models for it, which is where the models, their skins and
 * their extents come from (packages/pxl/shared/src/record/shipped.ts).
 */

import { bind_palette, type DecorKindBundle } from "./decor_layer.ts";
import { is_train_eligible, TRAIN_KIND, TRAIN_RULES } from "./train_kind.ts";

export const TRAIN_DECOR_BUNDLE: DecorKindBundle = bind_palette({
  kind: "train",
  seed: TRAIN_KIND,
  rules: TRAIN_RULES,
  is_eligible: is_train_eligible,
});
