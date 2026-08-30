// @pxl/web/render/layer/terrain/elevation.test.ts

import { assertEquals } from "@std/assert";

import type { I16Array } from "@xpute/core/kit/array.ts";
import type { Ground } from "@pxl/shared/bake/format.ts";
import { ELE_PADDED_SPAN } from "@pxl/shared/bake/constants.ts";
import { terrain_types } from "@pxl/shared/spec/cell/terrain.ts";

import { dense_ele_region, is_water_family_at, WATER_RECESS_CELLS } from "./elevation.ts";

const FLAT_ELE = 100;

/** A flat source raster: bicubic-interpolating a constant field returns
 * that same constant everywhere, with no edge/padding artifacts to account
 * for — isolates the recess logic from the interpolation math it sits on
 * top of. */
function flat_page_ele(): I16Array {
  return new Int16Array(ELE_PADDED_SPAN * ELE_PADDED_SPAN).fill(FLAT_ELE);
}

function empty_ground(span: number): Ground {
  return {
    land: new Uint8Array(span * span),
    water: new Uint8Array(span * span),
    structure: new Uint8Array(span * span),
    transport: new Uint8Array(span * span),
  };
}

Deno.test("dense_ele_region - a WATER-channel cell is recessed by WATER_RECESS_CELLS", () => {
  const span = 2;
  const ground = empty_ground(span);
  ground.water[0] = terrain_types.water.RIVER; // cell (0,0)

  const out = dense_ele_region(flat_page_ele(), 0, 0, span, ground);
  assertEquals(out[0], FLAT_ELE - WATER_RECESS_CELLS);
  assertEquals(out[1], FLAT_ELE); // untouched neighbor cell
});

Deno.test("dense_ele_region - land.SEA is recessed too, even though it's not the WATER channel", () => {
  const span = 2;
  const ground = empty_ground(span);
  ground.land[0] = terrain_types.land.SEA;

  const out = dense_ele_region(flat_page_ele(), 0, 0, span, ground);
  assertEquals(out[0], FLAT_ELE - WATER_RECESS_CELLS);
});

Deno.test("dense_ele_region - a structure over water (e.g. a bridge) is not recessed", () => {
  const span = 2;
  const ground = empty_ground(span);
  ground.water[0] = terrain_types.water.RIVER;
  ground.structure[0] = terrain_types.structure.BUILDING; // outranks water, same as adapter.ts's own priority

  const out = dense_ele_region(flat_page_ele(), 0, 0, span, ground);
  assertEquals(out[0], FLAT_ELE);
});

Deno.test("dense_ele_region - transport over water (e.g. a road) is not recessed", () => {
  const span = 2;
  const ground = empty_ground(span);
  ground.water[0] = terrain_types.water.LAKE;
  ground.transport[0] = terrain_types.transport.ROAD_DEFAULT;

  const out = dense_ele_region(flat_page_ele(), 0, 0, span, ground);
  assertEquals(out[0], FLAT_ELE);
});

Deno.test("dense_ele_region - omitting ground recesses nothing (existing callers with no ground data are unaffected)", () => {
  const out = dense_ele_region(flat_page_ele(), 0, 0, 2);
  assertEquals(out[0], FLAT_ELE);
  assertEquals(out[1], FLAT_ELE);
});

Deno.test("is_water_family_at - plain land is not water-family", () => {
  const ground = empty_ground(1);
  ground.land[0] = terrain_types.land.BARE;
  assertEquals(is_water_family_at(ground, 0), false);
});
