// @pxl/web/modules/world/decor/train_kind.ts

/**
 * A DecorKind's placement rules for occasional trains on rail lines — see
 * tree_kind.ts's own doc comment for why this file stays asset-free.
 *
 * RAIL_MAIN only (not RAIL_SUBWAY — that's underground, wrong context for
 * a surface-scattered prop). Same "align_transport" reasoning as
 * car_kind.ts applies: a train lying across the rail sideways would read
 * as derailed.
 */

import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import type { DecorEligibility, DecorKindSeed } from "./scatter.ts";

const TRAIN_KIND_TAG = 0x7ba1_5eed_0004_dec0n;

// Very sparse, deliberately — trains are meant to read as occasional, not
// as a train sitting on every visible stretch of track.
const TRAIN_DENSITY = 0.004;

// Matches train_decor.ts's TRAIN_MODEL_URLS length.
const TRAIN_VARIANT_COUNT = 4;

export const TRAIN_KIND: DecorKindSeed = {
  tag: TRAIN_KIND_TAG,
  density: TRAIN_DENSITY,
  variant_count: TRAIN_VARIANT_COUNT,
  orientation: "align_transport",
};

/** Only the transport channel matters — same reasoning as
 * car_kind.ts's own is_car_eligible. */
export const is_train_eligible: DecorEligibility = (_land, _water, _structure, transport) => transport === cube_types.transport.RAIL_MAIN;
