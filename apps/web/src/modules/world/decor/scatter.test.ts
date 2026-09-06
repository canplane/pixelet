// @pxl/web/modules/world/decor/scatter.test.ts

import { assert, assertEquals } from "@std/assert";

import type { f64 } from "@xpute/core/kit/word.ts";
import { GroundChannelKey } from "@pxl/shared/bake/format.ts";
import type { Ground } from "@pxl/shared/bake/format.ts";
import { PAGE_CELL_SPAN } from "@pxl/shared/bake/constants.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

import { type DecorKindSeed, scatter_decor_candidates } from "./scatter.ts";
import { is_tree_eligible } from "./tree_kind.ts";
import { is_car_eligible } from "./car_kind.ts";
import { is_train_eligible } from "./train_kind.ts";

// Small enough to keep these arrays cheap, big enough to hold a handful of
// test spans at tx0/ty0 > 0 without ever reading past the array.
const TEST_SPAN_CAP = 8;

function make_ground(land_fill: number = cube_types.common.VOID): Ground {
  const size = TEST_SPAN_CAP * PAGE_CELL_SPAN;
  return {
    [GroundChannelKey.LAND]: new Uint8Array(size).fill(land_fill),
    [GroundChannelKey.WATER]: new Uint8Array(size).fill(cube_types.common.VOID),
    [GroundChannelKey.STRUCTURE]: new Uint8Array(size).fill(cube_types.common.VOID),
    [GroundChannelKey.TRANSPORT]: new Uint8Array(size).fill(cube_types.common.VOID),
  };
}

const ALWAYS_PLACE: DecorKindSeed = { tag: 1n, density: 1, variant_count: 5, orientation: "random" };
const NEVER_PLACE: DecorKindSeed = { tag: 1n, density: 0, variant_count: 5, orientation: "random" };

Deno.test("scatter_decor_candidates - density 1 places on every eligible cell in range", () => {
  const ground = make_ground(cube_types.land.GREEN_LIGHT);
  const candidates = scatter_decor_candidates(ground, 0, 0, 4, 42n, ALWAYS_PLACE, is_tree_eligible);
  assertEquals(candidates.length, 16);
});

Deno.test("scatter_decor_candidates - density 0 never places, regardless of eligibility", () => {
  const ground = make_ground(cube_types.land.GREEN_LIGHT);
  const candidates = scatter_decor_candidates(ground, 0, 0, 4, 42n, NEVER_PLACE, is_tree_eligible);
  assertEquals(candidates.length, 0);
});

Deno.test("scatter_decor_candidates - ineligible land type (not green) is never a candidate even at density 1", () => {
  const ground = make_ground(cube_types.land.RESIDENTIAL);
  const candidates = scatter_decor_candidates(ground, 0, 0, 4, 42n, ALWAYS_PLACE, is_tree_eligible);
  assertEquals(candidates.length, 0);
});

Deno.test("scatter_decor_candidates - BARE land counts as eligible (this pack's actual bare-ground texture reads as plain grass)", () => {
  const ground = make_ground(cube_types.land.BARE);
  const candidates = scatter_decor_candidates(ground, 0, 0, 4, 42n, ALWAYS_PLACE, is_tree_eligible);
  assertEquals(candidates.length, 16);
});

Deno.test("scatter_decor_candidates - a non-VOID water/structure/transport cell overrides an otherwise-green cell", () => {
  const ground = make_ground(cube_types.land.GREEN_DEEP);
  const idx = 2 * PAGE_CELL_SPAN + 1; // cell (1, 2)
  ground[GroundChannelKey.WATER][idx] = cube_types.water.LAKE;
  const candidates = scatter_decor_candidates(ground, 0, 0, 4, 42n, ALWAYS_PLACE, is_tree_eligible);
  assertEquals(candidates.length, 15);
  assert(!candidates.some((c) => c.cell_x === 1 && c.cell_y === 2));
});

Deno.test("scatter_decor_candidates - deterministic: identical inputs always produce identical output", () => {
  const ground = make_ground(cube_types.land.GREEN_LIGHT);
  const kind: DecorKindSeed = { tag: 7n, density: 0.5, variant_count: 5, orientation: "random" };
  const a = scatter_decor_candidates(ground, 0, 0, 8, 1234n, kind, is_tree_eligible);
  const b = scatter_decor_candidates(ground, 0, 0, 8, 1234n, kind, is_tree_eligible);
  assertEquals(a, b);
  // Not every eligible cell placed (density < 1), but at least one did —
  // guards against a degenerate RNG that always rolls the same way.
  assert(a.length > 0 && a.length < 64);
});

Deno.test("scatter_decor_candidates - a different page seed reshuffles which cells get placed", () => {
  const ground = make_ground(cube_types.land.GREEN_LIGHT);
  const kind: DecorKindSeed = { tag: 7n, density: 0.5, variant_count: 5, orientation: "random" };
  const a = scatter_decor_candidates(ground, 0, 0, 8, 1234n, kind, is_tree_eligible);
  const b = scatter_decor_candidates(ground, 0, 0, 8, 5678n, kind, is_tree_eligible);
  assert(JSON.stringify(a) !== JSON.stringify(b));
});

Deno.test("scatter_decor_candidates - every candidate's variant is within [0, variant_count)", () => {
  const ground = make_ground(cube_types.land.GREEN_LIGHT);
  const kind: DecorKindSeed = { tag: 3n, density: 1, variant_count: 5, orientation: "random" };
  const candidates = scatter_decor_candidates(ground, 0, 0, 8, 99n, kind, is_tree_eligible);
  for (const c of candidates) {
    assert(c.variant >= 0 && c.variant < 5);
  }
});

const ALWAYS_PLACE_CAR: DecorKindSeed = { tag: 1n, density: 1, variant_count: 5, orientation: "align_transport" };

// align_transport also randomly flips the heading +PI (see scatter.ts's own
// doc comment — real two-way traffic doesn't all face the same way), so
// these check membership in {axis, axis + PI} rather than one exact value.
function assert_on_axis(rotation: f64, axis: f64): void {
  const normalized = ((rotation % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2);
  const a = ((axis % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2);
  const b = (a + Math.PI) % (Math.PI * 2);
  assert(Math.abs(normalized - a) < 1e-9 || Math.abs(normalized - b) < 1e-9, `expected ${normalized} to be ${a} or ${b}`);
}

Deno.test("scatter_decor_candidates - align_transport faces along X when the road runs east-west", () => {
  const ground = make_ground();
  const transport = ground[GroundChannelKey.TRANSPORT];
  // A horizontal 3-cell road segment through (2, 2): (1,2) (2,2) (3,2).
  for (const x of [1, 2, 3]) transport[2 * PAGE_CELL_SPAN + x] = cube_types.transport.ROAD_MOTORWAY;
  const candidates = scatter_decor_candidates(ground, 0, 0, 8, 42n, ALWAYS_PLACE_CAR, is_car_eligible);
  const at_2_2 = candidates.find((c) => c.cell_x === 2 && c.cell_y === 2);
  assert(at_2_2);
  assert_on_axis(at_2_2.rotation, Math.PI / 2);
});

Deno.test("scatter_decor_candidates - align_transport faces along Y when the road runs north-south", () => {
  const ground = make_ground();
  const transport = ground[GroundChannelKey.TRANSPORT];
  // A vertical 3-cell road segment through (2, 2): (2,1) (2,2) (2,3).
  for (const y of [1, 2, 3]) transport[y * PAGE_CELL_SPAN + 2] = cube_types.transport.ROAD_MOTORWAY;
  const candidates = scatter_decor_candidates(ground, 0, 0, 8, 42n, ALWAYS_PLACE_CAR, is_car_eligible);
  const at_2_2 = candidates.find((c) => c.cell_x === 2 && c.cell_y === 2);
  assert(at_2_2);
  assert_on_axis(at_2_2.rotation, 0);
});

Deno.test("scatter_decor_candidates - align_transport isn't thrown off by a single-cell diagonal stairstep on an otherwise-vertical rail", () => {
  const ground = make_ground();
  const transport = ground[GroundChannelKey.TRANSPORT];
  // A vertical rail at cell_x=2 from y=0 to y=6, plus one bogus east-jog
  // cell at (3, 3) — the kind of single-cell rasterization stairstep a
  // real (slightly off-axis) rail line produces. The immediate-neighbor-
  // only check this replaced would misread (2, 3) as horizontal purely
  // because of that jog; the wider vote should still find far more
  // vertical neighbors than the lone horizontal one and get it right.
  for (let y = 0; y <= 6; y++) transport[y * PAGE_CELL_SPAN + 2] = cube_types.transport.RAIL_MAIN;
  transport[3 * PAGE_CELL_SPAN + 3] = cube_types.transport.RAIL_MAIN;
  const candidates = scatter_decor_candidates(ground, 0, 0, 8, 42n, { ...ALWAYS_PLACE_CAR, orientation: "align_transport" }, is_train_eligible);
  const at_2_3 = candidates.find((c) => c.cell_x === 2 && c.cell_y === 3);
  assert(at_2_3);
  assert_on_axis(at_2_3.rotation, 0);
});

Deno.test("scatter_decor_candidates - align_transport headings aren't all identical along one straight road (the random flip actually varies)", () => {
  const ground = make_ground();
  const transport = ground[GroundChannelKey.TRANSPORT];
  for (let x = 0; x < 8; x++) transport[2 * PAGE_CELL_SPAN + x] = cube_types.transport.ROAD_MOTORWAY;
  const candidates = scatter_decor_candidates(ground, 0, 0, 8, 42n, ALWAYS_PLACE_CAR, is_car_eligible);
  const distinct = new Set(candidates.filter((c) => c.cell_y === 2).map((c) => c.rotation));
  assert(distinct.size > 1, "expected at least one flipped heading among 8 candidates on the same road");
});

Deno.test("scatter_decor_candidates - a road cell not ROAD_MOTORWAY is never a car candidate", () => {
  const ground = make_ground();
  ground[GroundChannelKey.TRANSPORT][2 * PAGE_CELL_SPAN + 2] = cube_types.transport.ROAD_DEFAULT;
  const candidates = scatter_decor_candidates(ground, 0, 0, 8, 42n, ALWAYS_PLACE_CAR, is_car_eligible);
  assertEquals(candidates.length, 0);
});
