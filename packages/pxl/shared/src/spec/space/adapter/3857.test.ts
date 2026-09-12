// @pxl/shared/spec/space/adapter/3857.test.ts

import { assertAlmostEquals, assertEquals } from "@std/assert";

import type { f64 } from "@xpute/core/kit/word.ts";

import { fvec, ivec, type PVF3, pvf3 } from "../pvf3/mod.ts";
import { FRAC_W } from "../pvf3/core.ts";

import * as epsg3857 from "./3857.ts";
import * as epsg4326 from "./4326.ts";
import type { G3857 } from "./3857.ts";

// project()/unproject() round-trip through a fixed-point PVF3 lattice (see
// pvf3/projection.ts), so exact equality isn't expected — only that the
// error stays within one lattice cell's worth of meters. 1u ≈ 2.389m (see
// M_PER_U in 3857.ts) and the fractional lane resolves well below that, so a
// tolerance far looser than a full unit (0.5m) confirms the round-trip isn't
// silently degenerating, without being so tight it starts asserting on the
// lattice's own intentional quantization.
const ROUND_TRIP_TOL_M = 0.5;
const WORLD_WIDTH_M = 40075016.68557849;
const HALF_WORLD_M = WORLD_WIDTH_M / 2;

function round_trip(x: f64, y: f64, z: f64): [f64, f64, f64] {
  const pvf3: PVF3 = { i: 0n, f: 0n };
  epsg3857.project(pvf3, x, y, z);
  // unproject() always writes all 3 slots (see 3857.ts) — G3857's `z?`
  // is only optional at the *input* end (project()'s default param), so
  // this fallback never actually triggers, it just satisfies the tuple's
  // own type.
  const out: G3857 = [0, 0, 0];
  epsg3857.unproject(out, pvf3.i, pvf3.f);
  return [out[0], out[1], out[2] ?? 0];
}

Deno.test("epsg3857 project/unproject - round-trips the origin", () => {
  const [x, y, z] = round_trip(0, 0, 0);
  assertAlmostEquals(x, 0, ROUND_TRIP_TOL_M);
  assertAlmostEquals(y, 0, ROUND_TRIP_TOL_M);
  assertAlmostEquals(z, 0, ROUND_TRIP_TOL_M);
});

Deno.test("epsg3857 project/unproject - round-trips a real-world-scale coordinate (roughly Seoul)", () => {
  // EPSG:3857 meters for lon/lat ~ (127.03, 37.50).
  const x_m = 14139274;
  const y_m = 4508140;
  const [x, y] = round_trip(x_m, y_m, 0);
  assertAlmostEquals(x, x_m, ROUND_TRIP_TOL_M);
  assertAlmostEquals(y, y_m, ROUND_TRIP_TOL_M);
});

Deno.test("epsg3857 project/unproject - round-trips a nonzero altitude, Mercator-corrected and un-corrected symmetrically", () => {
  const [, , z] = round_trip(14139274, 4508140, 50);
  assertAlmostEquals(z, 50, ROUND_TRIP_TOL_M);
});

Deno.test("epsg3857 project - wraps X across the antimeridian instead of growing unbounded", () => {
  const pvf3: PVF3 = { i: 0n, f: 0n };
  epsg3857.project(pvf3, WORLD_WIDTH_M * 1.5, 0, 0);
  const [x] = epsg3857.unproject([0, 0, 0], pvf3.i, pvf3.f);
  // 1.5 world-widths east wraps back to -0.5 world-widths (i.e. the
  // antimeridian's other side), not 1.5 world-widths.
  assertAlmostEquals(x, -HALF_WORLD_M, ROUND_TRIP_TOL_M);
});

Deno.test("epsg3857 scale - maps one complete world onto the full 24-bit X lane", () => {
  assertAlmostEquals(epsg3857.M_PER_U, WORLD_WIDTH_M / 2 ** 24, 1e-12);
  assertAlmostEquals(epsg3857.U_PER_M, 2 ** 24 / WORLD_WIDTH_M, 1e-12);
});

Deno.test("epsg3857 project - keeps the clamped north edge below the signed Y seam", () => {
  const coord: PVF3 = { i: 0n, f: 0n };
  epsg3857.project(coord, 0, HALF_WORLD_M, 0);

  assertEquals(ivec.Y(coord.i), (1n << 23n) - 1n);
  assertEquals(fvec.Y(coord.f), (1n << BigInt(FRAC_W)) - 1n);

  const [, y] = epsg3857.unproject([0, 0, 0], coord.i, coord.f);
  assertEquals(y > 0, true);
  assertAlmostEquals(y, HALF_WORLD_M, 0.01);
});

Deno.test("epsg3857 project - PVF3 X subtraction takes the short antimeridian path", () => {
  const east: PVF3 = { i: 0n, f: 0n };
  const west: PVF3 = { i: 0n, f: 0n };
  const delta: PVF3 = { i: 0n, f: 0n };

  epsg3857.project(east, HALF_WORLD_M - 1, 0, 0);
  epsg3857.project(west, -HALF_WORLD_M + 1, 0, 0);
  pvf3.sub(delta, west, east);

  assertAlmostEquals(pvf3.to_real(pvf3.X(delta)) * epsg3857.M_PER_U, 2, 0.01);
});

Deno.test("epsg3857 page bbox - preserves the exclusive positive world boundary", () => {
  const page_idx = ivec.gather(8191n, 8191n, 0n);
  const [, max] = epsg3857.get_page_bbox(page_idx);

  assertAlmostEquals(max[0], HALF_WORLD_M, 1e-8);
  assertAlmostEquals(max[1], HALF_WORLD_M, 1e-8);

  const [, max_geodetic] = epsg4326.get_page_bbox(page_idx);
  assertAlmostEquals(max_geodetic[0], 180, 1e-10);
});

Deno.test("epsg3857 Z_SCALE - is 1 at the equator (no Mercator distortion)", () => {
  assertAlmostEquals(epsg3857.Z_SCALE(0), 1, 1e-6);
});

Deno.test("epsg3857 Z_SCALE - grows away from the equator (Mercator's characteristic distortion)", () => {
  const at_equator = epsg3857.Z_SCALE(0);
  // ~37.5N in EPSG:3857 meters.
  const at_high_lat = epsg3857.Z_SCALE(4508140);
  assertEquals(at_high_lat > at_equator, true);
});

Deno.test("epsg3857 Z_SCALE - constant within one L3 page (the documented coarse-scale contract, not a bug)", () => {
  // Z_SCALE deliberately snaps to the *page base* before evaluating scale
  // (see its doc comment in 3857.ts) so terrain/object/baking never drift
  // from each other by mixing slightly different per-point scales — two
  // points a few meters apart, comfortably inside the same page, must
  // return bit-identical scale, not just approximately equal.
  const base = 4508140;
  assertEquals(epsg3857.Z_SCALE(base), epsg3857.Z_SCALE(base + 3));
});
