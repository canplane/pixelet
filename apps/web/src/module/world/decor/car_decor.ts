// @pxl/web/module/world/decor/car_decor.ts

/**
 * The car kind as a bundle: car_kind.ts's placement rules bound to the
 * palette's models for it, which is where the models, their skins and
 * their extents come from (packages/pxl/shared/src/record/shipped.ts).
 */

import { bind_palette, type DecorKindBundle } from "./decor_layer.ts";
import { CAR_KIND, CAR_RULES, is_car_eligible } from "./car_kind.ts";

export const CAR_DECOR_BUNDLE: DecorKindBundle = bind_palette({
  kind: "car",
  seed: CAR_KIND,
  rules: CAR_RULES,
  is_eligible: is_car_eligible,
});
