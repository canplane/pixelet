// @pxl/web/modules/world/decor/tree_decor.ts

/**
 * Wires tree_kind.ts's placement rules to their actual model assets —
 * split out of tree_kind.ts specifically so that file (and anything that
 * tests against it, e.g. scatter.test.ts) never has to resolve a Vite-only
 * `?url` asset import through plain `deno test`/`deno check`, the same
 * problem assets/characters/variants.ts would hit if a test ever imported
 * it. world.tsx is this file's only real consumer.
 *
 * Mixes two Kenney packs into one kind's variant list — fantasy-town-kit's
 * five trees plus graveyard-kit's four pines (two plain green, two "fall"/
 * ochre) — each variant carrying its own skin since the two packs don't
 * share a colormap atlas (see instanced_model.ts's own doc comment on why
 * that's supported).
 */

import { TREE_MODEL_URLS, TREE_SKIN_URL } from "@pxl/shared/assets/decor/trees.ts";
import { PINE_MODEL_URLS, PINE_SKIN_URL } from "@pxl/shared/assets/decor/graveyard_trees.ts";
import type { DecorKindBundle } from "./decor_layer.tsx";
import { is_tree_eligible, TREE_KIND } from "./tree_kind.ts";

const TREE_VARIANTS = [
  ...TREE_MODEL_URLS.map((model_url) => ({ model_url, skin_url: TREE_SKIN_URL })),
  ...PINE_MODEL_URLS.map((model_url) => ({ model_url, skin_url: PINE_SKIN_URL })),
];

if (TREE_VARIANTS.length !== TREE_KIND.variant_count) {
  throw new Error(
    `decor: TREE_KIND.variant_count (${TREE_KIND.variant_count}) must match the combined tree variant count (${TREE_VARIANTS.length}) — update tree_kind.ts's TREE_VARIANT_COUNT`,
  );
}

export const TREE_DECOR_BUNDLE: DecorKindBundle = {
  seed: TREE_KIND,
  is_eligible: is_tree_eligible,
  variants: TREE_VARIANTS,
};
