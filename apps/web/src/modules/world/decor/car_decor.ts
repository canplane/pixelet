// @pxl/web/modules/world/decor/car_decor.ts

// Wires car_kind.ts's placement rules to their actual model assets — see
// tree_decor.ts's own doc comment for why assets live in a separate file.

import { CAR_MODEL_URLS, CAR_SKIN_URL } from "@pxl/shared/assets/decor/cars.ts";
import { bind_palette, type DecorKindBundle } from "./decor_layer.ts";
import { CAR_KIND, is_car_eligible } from "./car_kind.ts";

const CAR_VARIANTS = CAR_MODEL_URLS.map((model_url) => ({ model_url, skin_url: CAR_SKIN_URL }));

if (CAR_VARIANTS.length !== CAR_KIND.variant_count) {
  throw new Error(
    `decor: CAR_KIND.variant_count (${CAR_KIND.variant_count}) must match CAR_MODEL_URLS.length (${CAR_VARIANTS.length}) — update car_kind.ts's CAR_VARIANT_COUNT`,
  );
}

export const CAR_DECOR_BUNDLE: DecorKindBundle = bind_palette({
  kind: "car",
  seed: CAR_KIND,
  is_eligible: is_car_eligible,
  variants: CAR_VARIANTS,
});
