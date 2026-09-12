// @pxl/web/module/world/decor/boat_decor.ts

/**
 * The boat kind as a bundle: boat_kind.ts's placement rules bound to the
 * palette's models for it, which is where the models, their skins and
 * their extents come from (packages/pxl/shared/src/record/shipped.ts).
 */

import { bind_palette, type DecorKindBundle } from "./decor_layer.ts";
import { BOAT_KIND, is_boat_eligible } from "./boat_kind.ts";

export const BOAT_DECOR_BUNDLE: DecorKindBundle = bind_palette({
  kind: "boat",
  seed: BOAT_KIND,
  is_eligible: is_boat_eligible,
});
