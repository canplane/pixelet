// @pxl/web/module/world/decor/car_kind.ts

/**
 * A DecorKind's placement rules for ambient cars on major roads — see
 * tree_kind.ts's own doc comment for why this file stays asset-free.
 *
 * Motorway/primary/secondary only, deliberately: ordinary neighborhood
 * streets and pedestrian ways (ROAD_DEFAULT, PEDESTRIAN) are dense and
 * everywhere a user might want to build, and a car parked in the middle of
 * one reads as "in the way" far more than the same car sitting on a major
 * road's shoulder does. "align_transport" orientation (see scatter.ts's own
 * doc comment) is what keeps these lined up with the road instead of
 * facing a random direction, which would otherwise read as a crash, not
 * traffic.
 */

import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { type DecorEligibility, type DecorKindSeed, type DecorRules, eligibility_of } from "./scatter.ts";

const CAR_KIND_TAG = 0xca5_eed0_0003_dec0n;

// Much sparser than trees/boats: a highway with a car every couple of
// cells would read as gridlock, not ambient traffic.
const CAR_DENSITY = 0.004;

// The palette's model rows for "car"; car_decor.ts asserts the two agree.
const CAR_VARIANT_COUNT = 5;

export const CAR_KIND: DecorKindSeed = {
  tag: CAR_KIND_TAG,
  density: CAR_DENSITY,
  variant_count: CAR_VARIANT_COUNT,
  orientation: "align_transport",
};

/** Only the transport channel matters here: per render/layer/terrain/
 * adapter.ts's own ground_material_at precedence, transport always wins
 * last, so a cell whose transport is one of these renders (and therefore
 * counts) as that road type regardless of whatever land/water/structure
 * value happens to sit underneath it. ROAD_DEFAULT and PEDESTRIAN are
 * deliberately excluded — see this file's own doc comment. */
export const CAR_RULES: DecorRules = {
  land: "any",
  water: "any",
  structure: "any",
  transport: [cube_types.transport.ROAD_MOTORWAY, cube_types.transport.ROAD_PRIMARY, cube_types.transport.ROAD_SECONDARY],
};
export const is_car_eligible: DecorEligibility = eligibility_of(CAR_RULES);
