// @pxl/web/modules/world/sched/priority.test.ts

import { assertAlmostEquals, assertEquals } from "@std/assert";
import * as THREE from "three";

import { EMPTY, HOLDS_SLOT, IN_FRUSTUM, type PriorityLanes } from "./chunk_table.ts";
import { AGE_HALF_MS, lead_point, priority_order } from "./priority.ts";

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

type Item = { name?: string; in_frustum: boolean; lead_distance2: number; empty?: boolean; holds_slot?: boolean; pending_since?: number };

/** A few slots' worth of lanes, and their order under `priority_order`. */
function ordered(items: Item[], now: number): Item[] {
  const lanes: PriorityLanes = {
    flags: new Uint8Array(items.length),
    lead_distance2: new Float64Array(items.length),
    pending_since: new Float64Array(items.length),
  };
  items.forEach((item, i) => {
    lanes.flags[i] = (item.in_frustum ? IN_FRUSTUM : 0) | (item.empty ? EMPTY : 0) | (item.holds_slot ? HOLDS_SLOT : 0);
    lanes.lead_distance2[i] = item.lead_distance2;
    lanes.pending_since[i] = item.pending_since ?? -1;
  });
  const slots = Uint32Array.from(items.keys());
  slots.sort(priority_order(lanes, now));
  return Array.from(slots, (slot) => items[slot]);
}

Deno.test("priority_order - slot holders, then what is on screen, then nearest first", () => {
  const items = [
    { name: "far refine", in_frustum: true, lead_distance2: 900 },
    { name: "near hidden", in_frustum: false, lead_distance2: 1, empty: true },
    { name: "holder", in_frustum: false, lead_distance2: 5000, holds_slot: true },
    { name: "near refine", in_frustum: true, lead_distance2: 4 },
    { name: "far hole", in_frustum: true, lead_distance2: 2500, empty: true },
  ];
  assertEquals(ordered(items, 0).map((i) => i.name), ["holder", "near refine", "far refine", "far hole", "near hidden"]);
});

Deno.test("priority_order - a near chunk that is merely coarse goes before a far one that is empty", () => {
  // The other way round, every hole anywhere on screen outranked every
  // refinement, and ground a few metres away stayed blocky while the fog edge
  // filled in.
  const near_coarse = { name: "near, coarse", in_frustum: true, lead_distance2: 100 };
  const far_hole = { name: "far, empty", in_frustum: true, lead_distance2: 1e6, empty: true };
  assertEquals(ordered([far_hole, near_coarse], 0).map((i) => i.name), ["near, coarse", "far, empty"]);
});

Deno.test("priority_order - at the same distance, appearing beats sharpening", () => {
  const coarse = { name: "coarse", in_frustum: true, lead_distance2: 400 };
  const empty = { name: "empty", in_frustum: true, lead_distance2: 400, empty: true };
  assertEquals(ordered([coarse, empty], 0).map((i) => i.name), ["empty", "coarse"]);
});

Deno.test("priority_order - a waiting chunk overtakes a fresh nearer one after log2 of how much farther it is", () => {
  // Eight times farther: three half-lives.
  const first_at = (now: number) => {
    const far = { name: "far, waiting", in_frustum: true, lead_distance2: 64 * 64, pending_since: 0 };
    const near = { name: "near, fresh", in_frustum: true, lead_distance2: 8 * 8, pending_since: now };
    return ordered([near, far], now)[0].name;
  };
  assertEquals(first_at(3 * AGE_HALF_MS - 1), "near, fresh");
  assertEquals(first_at(3 * AGE_HALF_MS + 1), "far, waiting");
});

Deno.test("priority_order - off screen, the farthest release goes first", () => {
  // A release gives memory back, so it takes it from what is least likely to
  // be wanted again. Releasing nearest-first left far chunks holding stale
  // meshes through a short zoom-in, drawn as a ring on the next zoom-out.
  const near = { name: "near", in_frustum: false, lead_distance2: 4 };
  const far = { name: "far", in_frustum: false, lead_distance2: 900 };
  assertEquals(ordered([near, far], 0).map((i) => i.name), ["far", "near"]);
});

Deno.test("priority_order - a wait never lifts an item onto the screen", () => {
  const aged_release = { in_frustum: false, lead_distance2: 1, pending_since: 0 };
  const fresh_build = { in_frustum: true, lead_distance2: 1e12, pending_since: 1e6 };
  assertEquals(ordered([aged_release, fresh_build], 1e6)[0], fresh_build);
});

Deno.test("priority_order - items that began waiting together keep nearest-first", () => {
  const a = { in_frustum: true, lead_distance2: 9, pending_since: 100 };
  const b = { in_frustum: true, lead_distance2: 4, pending_since: 100 };
  assertEquals(ordered([a, b], 5000)[0], b);
});
