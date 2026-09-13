// @pxl/web/module/world/decor/decor_registry.ts

/**
 * Every decor kind's own bundle, in one place. world.tsx's own mount list
 * (ChunkStage's decor_bundles prop) and marker/hover_cursor.tsx's own
 * ghost-shape resolution (which bundle does a given decor_id's silhouette
 * come from) both need this same lookup — one shared registry instead of
 * two call sites independently assembling "every decor kind" and risking
 * drifting out of sync with each other.
 */

import type { DecorId } from "@pxl/shared/spec/cell/mod.ts";
import { type DecorKindBundle, teach_decor_kinds } from "./decor_layer.ts";
import { TREE_DECOR_BUNDLE } from "./tree_decor.ts";
import { BOAT_DECOR_BUNDLE } from "./boat_decor.ts";
import { CAR_DECOR_BUNDLE } from "./car_decor.ts";
import { TRAIN_DECOR_BUNDLE } from "./train_decor.ts";

export const DECOR_BUNDLES: readonly DecorKindBundle[] = [TREE_DECOR_BUNDLE, BOAT_DECOR_BUNDLE, CAR_DECOR_BUNDLE, TRAIN_DECOR_BUNDLE];
teach_decor_kinds(DECOR_BUNDLES);

const BY_DECOR_ID = new Map<DecorId, DecorKindBundle>();
for (const bundle of DECOR_BUNDLES) {
  if (bundle.decor_id) BY_DECOR_ID.set(bundle.decor_id, bundle);
}

/** Which bundle a user-placeable decor_id (objects.ts's object_types.decor)
 * belongs to — null for a decor_id somehow missing its own bundle, which
 * would itself be a real bug (every decor_id-carrying bundle registers
 * here), not an expected runtime case. */
export function decor_bundle_for(decor_id: DecorId): DecorKindBundle | null {
  return BY_DECOR_ID.get(decor_id) ?? null;
}
