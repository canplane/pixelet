// @pxl/web/modules/world/decor/boat_decor.ts

// Wires boat_kind.ts's placement rules to their actual model assets — see
// tree_decor.ts's own doc comment for why assets live in a separate file.

import { BOAT_MODEL_URLS, BOAT_SKIN_URL } from "@pxl/shared/assets/decor/boats.ts";
import { bind_palette, type DecorKindBundle } from "./decor_layer.ts";
import { BOAT_KIND, is_boat_eligible } from "./boat_kind.ts";

const BOAT_VARIANTS = BOAT_MODEL_URLS.map((model_url) => ({ model_url, skin_url: BOAT_SKIN_URL }));

if (BOAT_VARIANTS.length !== BOAT_KIND.variant_count) {
  throw new Error(
    `decor: BOAT_KIND.variant_count (${BOAT_KIND.variant_count}) must match BOAT_MODEL_URLS.length (${BOAT_VARIANTS.length}) — update boat_kind.ts's BOAT_VARIANT_COUNT`,
  );
}

export const BOAT_DECOR_BUNDLE: DecorKindBundle = bind_palette({
  kind: "boat",
  seed: BOAT_KIND,
  is_eligible: is_boat_eligible,
  variants: BOAT_VARIANTS,
});
