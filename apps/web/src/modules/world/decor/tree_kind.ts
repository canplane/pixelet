// @pxl/web/modules/world/decor/tree_kind.ts

/**
 * The first concrete DecorKind's placement rules — trees on green/forest
 * terrain. Everything kind-specific (which cells count as eligible, how
 * dense) lives here; decor_layer.ts and scatter.ts stay generic so a later
 * kind (a fence along a road edge, a candle inside a structure, ...) is
 * just another file shaped like this one, not a change to either of those.
 *
 * Deliberately asset-free (no .glb/.png imports) — see tree_decor.ts's own
 * doc comment for why those live in a separate file instead of here.
 */

import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import type { DecorEligibility, DecorKindSeed } from "./scatter.ts";

// Arbitrary, fixed — only has to stay stable across sessions (see
// DecorKindSeed's own doc comment); reusing @xpute/core's own SEED_SALT
// shape (a big odd constant) rather than a small number so it mixes well
// even before combining with a per-page seed.
const TREE_KIND_TAG = 0x7ee5eed7eec0de1n;

// Moderate, deliberately sparse — a scattered handful of trees reading as
// "this hillside has some trees on it", not a solid forest wall at 1u
// spacing (which a much higher density would read as, given every eligible
// cell is only 1u wide).
const TREE_DENSITY = 0.04;

// Matches tree_decor.ts's combined variant list length (5 fantasy-town-kit
// + 4 graveyard-kit pines) — kept as a plain literal rather than importing
// those assets just to read .length, so this file (and everything that
// tests against it, e.g. scatter.test.ts) never pulls in a Vite-only `?url`
// asset import. tree_decor.ts asserts this stays in sync at import time.
const TREE_VARIANT_COUNT = 9;

export const TREE_KIND: DecorKindSeed = {
  tag: TREE_KIND_TAG,
  density: TREE_DENSITY,
  variant_count: TREE_VARIANT_COUNT,
  orientation: "random",
};

/** Green/forest land, and only where nothing else (water, a structure, a
 * road) already claims the cell — the same "what actually renders there"
 * precedence render/layer/terrain/adapter.ts's ground_material_at applies,
 * since a cell whose land channel says GREEN_LIGHT but whose transport
 * channel is a road is a road, not grass.
 *
 * BARE is included alongside the two explicit GREEN_* types even though its
 * own name says "bare ground": it's the fallback land type for any cell
 * with no real landuse tag (see render/layer/terrain/adapter.ts's own
 * ground_material_at, which uses it as the base before any channel
 * override), and this pack's actual land_bare.png texture is a grassy
 * yellow-green speckle nearly indistinguishable from land_green_light.png —
 * visually it *is* the plain grass most of an untagged map reads as, not
 * dirt. SPORTS was considered too (its own texture leans mint-green) but
 * left out: it's a maintained field, not natural ground, and reads wrong
 * with trees scattered across it. */
export const is_tree_eligible: DecorEligibility = (land, water, structure, transport) => {
  if (water !== cube_types.common.VOID) return false;
  if (structure !== cube_types.common.VOID) return false;
  if (transport !== cube_types.common.VOID) return false;
  return land === cube_types.land.GREEN_LIGHT || land === cube_types.land.GREEN_DEEP || land === cube_types.land.BARE;
};
