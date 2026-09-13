// @pxl/web/module/world/sched/priority.test.ts

import { assertAlmostEquals } from "@std/assert";
import * as THREE from "three";

import { lead_point } from "./priority.ts";

const camera = new THREE.Vector3(0, -300, 200);

Deno.test("lead_point - at rest the target sits at the world origin, so the lead is the camera", () => {
  // The frame has caught up: it is translated so the target is at the origin.
  const frame = new THREE.Matrix4().makeTranslation(-1000, -2000, -30);
  const lead = lead_point(new THREE.Vector3(), camera, 1000, 2000, 30, frame);
  assertAlmostEquals(lead.distanceTo(camera), 0, 1e-9);
});

Deno.test("lead_point - mid-pan, the lead is ahead of the camera by the travel still to come", () => {
  // Frame still showing (1000, 2000) while the target moved on to (1100, 2000).
  const frame = new THREE.Matrix4().makeTranslation(-1000, -2000, -30);
  const lead = lead_point(new THREE.Vector3(), camera, 1100, 2000, 30, frame);
  assertAlmostEquals(lead.x - camera.x, 100, 1e-9);
  assertAlmostEquals(lead.y - camera.y, 0, 1e-9);
});

Deno.test("lead_point - the frame's rotation carries through, so 'ahead' matches the screen", () => {
  // A quarter turn of azimuth: local +X is world +Y.
  const frame = new THREE.Matrix4().makeRotationZ(Math.PI / 2).multiply(new THREE.Matrix4().makeTranslation(-1000, -2000, -30));
  const lead = lead_point(new THREE.Vector3(), camera, 1100, 2000, 30, frame);
  assertAlmostEquals(lead.x - camera.x, 0, 1e-9);
  assertAlmostEquals(lead.y - camera.y, 100, 1e-9);
});
