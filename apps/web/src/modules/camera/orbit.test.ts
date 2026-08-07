// @pxl/web/modules/camera/orbit.test.ts

import { assertAlmostEquals, assertEquals } from "@std/assert";

import { damp_alpha, orbit_local_offset, rotate2 } from "./orbit.ts";

Deno.test("orbit_local_offset - polar=0 looks straight down: all offset is vertical", () => {
  const { horiz_dist, vert_dist } = orbit_local_offset(100, 0);
  assertAlmostEquals(horiz_dist, 0);
  assertAlmostEquals(vert_dist, 100);
});

Deno.test("orbit_local_offset - polar=pi/2 looks at the horizon: all offset is horizontal", () => {
  const { horiz_dist, vert_dist } = orbit_local_offset(100, Math.PI / 2);
  assertAlmostEquals(horiz_dist, 100);
  assertAlmostEquals(vert_dist, 0, 1e-9);
});

Deno.test("orbit_local_offset - offset magnitude always equals dist regardless of polar", () => {
  for (const polar of [0, 0.3, 0.7, 1.0, 1.5]) {
    const { horiz_dist, vert_dist } = orbit_local_offset(50, polar);
    assertAlmostEquals(Math.hypot(horiz_dist, vert_dist), 50);
  }
});

Deno.test("rotate2 - identity at angle=0", () => {
  const r = rotate2(3, 4, 0);
  assertAlmostEquals(r.x, 3);
  assertAlmostEquals(r.y, 4);
});

Deno.test("rotate2 - quarter turn maps +X onto +Y", () => {
  const r = rotate2(1, 0, Math.PI / 2);
  assertAlmostEquals(r.x, 0, 1e-9);
  assertAlmostEquals(r.y, 1, 1e-9);
});

Deno.test("rotate2 - preserves vector length (pure rotation, no scaling)", () => {
  const r = rotate2(3, -7, 1.234);
  assertAlmostEquals(Math.hypot(r.x, r.y), Math.hypot(3, -7));
});

Deno.test("rotate2 - rotating by -angle undoes rotating by +angle (the inverse-transform relationship environment.tsx's sky-dome sun rotation relies on)", () => {
  const angle = 0.9;
  const forward = rotate2(5, -2, angle);
  const back = rotate2(forward.x, forward.y, -angle);
  assertAlmostEquals(back.x, 5);
  assertAlmostEquals(back.y, -2);
});

Deno.test("damp_alpha - zero elapsed time never moves the damped value", () => {
  assertEquals(damp_alpha(10, 0), 0);
});

Deno.test("damp_alpha - grows toward 1 as delta grows, without overshooting it", () => {
  const a = damp_alpha(10, 0.05);
  const b = damp_alpha(10, 1);
  assertEquals(a < b, true);
  assertEquals(b < 1, true);
});

Deno.test("damp_alpha - underflows to exactly 1 once delta is large enough (exp() has hit float precision's floor, not a bug)", () => {
  assertEquals(damp_alpha(10, 100), 1);
});

Deno.test("damp_alpha - a larger rate reaches the same alpha in less time", () => {
  const slow = damp_alpha(5, 0.1);
  const fast = damp_alpha(20, 0.1);
  assertEquals(fast > slow, true);
});
