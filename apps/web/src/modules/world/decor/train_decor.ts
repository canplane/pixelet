// @pxl/web/modules/world/decor/train_decor.ts

// Wires train_kind.ts's placement rules to their actual model assets — see
// tree_decor.ts's own doc comment for why assets live in a separate file.

import { TRAIN_MODEL_URLS, TRAIN_SKIN_URL } from "@pxl/shared/assets/decor/trains.ts";
import type { DecorKindBundle } from "./decor_layer.ts";
import { is_train_eligible, TRAIN_KIND } from "./train_kind.ts";

const TRAIN_VARIANTS = TRAIN_MODEL_URLS.map((model_url) => ({ model_url, skin_url: TRAIN_SKIN_URL }));

if (TRAIN_VARIANTS.length !== TRAIN_KIND.variant_count) {
  throw new Error(
    `decor: TRAIN_KIND.variant_count (${TRAIN_KIND.variant_count}) must match TRAIN_MODEL_URLS.length (${TRAIN_VARIANTS.length}) — update train_kind.ts's TRAIN_VARIANT_COUNT`,
  );
}

export const TRAIN_DECOR_BUNDLE: DecorKindBundle = {
  seed: TRAIN_KIND,
  is_eligible: is_train_eligible,
  variants: TRAIN_VARIANTS,
};
