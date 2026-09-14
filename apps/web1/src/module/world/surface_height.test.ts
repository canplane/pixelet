// @pxl/web/module/world/surface_height.test.ts

import { assertAlmostEquals, assertEquals } from "@std/assert";

import { pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { f64 } from "@xpute/core/kit/word.ts";

import { place } from "./object/store.ts";
import { surface_z_from_terrain } from "./surface_height.ts";

// place()/get_objects_near() are module-global state (object/store.ts),
// not scoped per-test — every test below uses its own well-separated local
// X so its own placed objects never land in another test's ±1-cell query
// neighborhood (U_PER_CELL is 1, so 10 units apart is comfortably clear).
const MAP_ORIGIN: IVec = 0n;

function put_block(x: f64, y: f64, z: f64): void {
  const corner = pvf3.gather({ i: 0n, f: 0n }, pvf3.from_real(x), pvf3.from_real(y), pvf3.from_real(z));
  place(corner.i, corner.f, 1, "red");
}

Deno.test("surface_z_from_terrain - bare terrain with no objects returns terrain_ele unchanged", () => {
  const z = surface_z_from_terrain(MAP_ORIGIN, 1000, 1000, 3.5, 3.5);
  assertEquals(z, 3.5);
});

Deno.test("surface_z_from_terrain - single object standing on the ground: lands on its top, not inside it", () => {
  const x = 1010, y = 1000;
  put_block(x, y, 0);
  const z = surface_z_from_terrain(MAP_ORIGIN, x, y, 0, 0);
  assertAlmostEquals(z, 1);
});

Deno.test("surface_z_from_terrain - tall contiguous stack lands on its true top (regression: used to always read as only 1 level tall)", () => {
  const x = 1020, y = 1000;
  for (let level = 0; level < 5; level++) put_block(x, y, level);
  const z = surface_z_from_terrain(MAP_ORIGIN, x, y, 0, 0);
  assertAlmostEquals(z, 5);
});

// Shared fixture for the gapped-structure tests below, matching the user's
// own diagram: x,O,x,O,x,x (solid, gap, solid, gap, solid, solid) — a
// 2-tall contiguous segment (levels 4-5) sitting above a lone block
// (level 0), with a real gap on either side of the lone block.
function put_gapped_structure(x: f64, y: f64): void {
  put_block(x, y, 0); // lone block, top = 1
  put_block(x, y, 2); // lone block, top = 3 — gap between 1 and 2
  put_block(x, y, 4); // bottom of the 2-tall segment
  put_block(x, y, 5); // flush on top of level 4 — segment top = 6
}

Deno.test("surface_z_from_terrain - gapped structure: hint at the ground stops at the first block, doesn't skip the gap", () => {
  const x = 1030, y = 1000;
  put_gapped_structure(x, y);
  const z = surface_z_from_terrain(MAP_ORIGIN, x, y, 0, 0);
  assertAlmostEquals(z, 1);
});

Deno.test("surface_z_from_terrain - gapped structure: hint inside the lower gap snaps UP to the next platform's top", () => {
  const x = 1040, y = 1000;
  put_gapped_structure(x, y);
  const z = surface_z_from_terrain(MAP_ORIGIN, x, y, 0, 1.4);
  assertAlmostEquals(z, 3);
});

Deno.test("surface_z_from_terrain - gapped structure: hint inside the upper segment climbs to its true (contiguous) top", () => {
  const x = 1050, y = 1000;
  put_gapped_structure(x, y);
  const z = surface_z_from_terrain(MAP_ORIGIN, x, y, 0, 4.5);
  assertAlmostEquals(z, 6);
});
