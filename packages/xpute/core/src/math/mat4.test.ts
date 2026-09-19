// @xpute/core/math/mat4.test.ts

import { assert, assertAlmostEquals } from "@std/assert";

import { mat4, mat4_frustum_planes, mat4_invert, mat4_look_at, mat4_mul, mat4_perspective, mat4_rotation_z, mat4_transform_point, mat4_translation } from "./mat4.ts";

Deno.test("mat4 - a matrix times its inverse is the identity", () => {
  const m = mat4_mul(mat4(), mat4_rotation_z(mat4(), 0.7), mat4_translation(mat4(), 5, -3, 2));
  const p = mat4_mul(mat4(), m, mat4_invert(mat4(), m));
  const identity = mat4();
  for (let k = 0; k < 16; k++) assertAlmostEquals(p[k], identity[k], 1e-12);
});

Deno.test("mat4 - look_at puts the target on the camera's -Z axis, up on +Y", () => {
  const v = mat4_look_at(mat4(), { x: 0, y: -30, z: 40 }, { x: 0, y: 0, z: 0 }, { x: 0, y: 0, z: 1 });
  const target = mat4_transform_point({ x: 0, y: 0, z: 0 }, v, 0, 0, 0);
  assertAlmostEquals(target.x, 0, 1e-12);
  assertAlmostEquals(target.y, 0, 1e-12);
  assertAlmostEquals(target.z, -50, 1e-12);
  // A point above the target is up on screen.
  const above = mat4_transform_point({ x: 0, y: 0, z: 0 }, v, 0, 0, 1);
  assert(above.y > 0);
});

Deno.test("mat4 - the frustum's planes hold the target and reject what is behind the eye", () => {
  const view = mat4_look_at(mat4(), { x: 0, y: -30, z: 40 }, { x: 0, y: 0, z: 0 }, { x: 0, y: 0, z: 1 });
  const proj = mat4_perspective(mat4(), Math.PI / 3, 1.6, 0.1, 1000);
  const planes = mat4_frustum_planes(new Float64Array(24), mat4_mul(mat4(), proj, view));
  const inside = (x: number, y: number, z: number) => {
    for (let k = 0; k < 24; k += 4) if (planes[k] * x + planes[k + 1] * y + planes[k + 2] * z + planes[k + 3] < 0) return false;
    return true;
  };
  assert(inside(0, 0, 0));
  assert(!inside(0, -60, 80));
  assert(!inside(0, 3000, 0));
});
