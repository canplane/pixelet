// @pxl/web/module/world/sched/priority.test.ts

import { assertAlmostEquals } from "@std/assert";

import { mat4, mat4_mul, mat4_rotation_z, mat4_translation } from "@xpute/core/kit/mat4.ts";

import { lead_point } from "./priority.ts";

const camera = { x: 0, y: -300, z: 200 };
const out = () => ({ x: 0, y: 0, z: 0 });

Deno.test("lead_point - at rest the target sits at the world origin, so the lead is the camera", () => {
  // The frame has caught up: it is translated so the target is at the origin.
  const frame = mat4_translation(mat4(), -1000, -2000, -30);
  const lead = lead_point(out(), camera, 1000, 2000, 30, frame);
  assertAlmostEquals(Math.hypot(lead.x - camera.x, lead.y - camera.y, lead.z - camera.z), 0, 1e-9);
});

Deno.test("lead_point - mid-pan, the lead is ahead of the camera by the travel still to come", () => {
  // Frame still showing (1000, 2000) while the target moved on to (1100, 2000).
  const frame = mat4_translation(mat4(), -1000, -2000, -30);
  const lead = lead_point(out(), camera, 1100, 2000, 30, frame);
  assertAlmostEquals(lead.x - camera.x, 100, 1e-9);
  assertAlmostEquals(lead.y - camera.y, 0, 1e-9);
});

Deno.test("lead_point - the frame's rotation carries through, so 'ahead' matches the screen", () => {
  // A quarter turn of azimuth: local +X is world +Y.
  const frame = mat4_mul(mat4(), mat4_rotation_z(mat4(), Math.PI / 2), mat4_translation(mat4(), -1000, -2000, -30));
  const lead = lead_point(out(), camera, 1100, 2000, 30, frame);
  assertAlmostEquals(lead.x - camera.x, 0, 1e-9);
  assertAlmostEquals(lead.y - camera.y, 100, 1e-9);
});
