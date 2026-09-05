// @pxl/web/modules/world/decor/instanced_model.test.ts

import { assert, assertAlmostEquals, assertEquals } from "@std/assert";
import * as THREE from "three";

import { bake_decor_instances, type ModelInstanceTransform } from "./instanced_model.ts";

Deno.test("bake_decor_instances - zero instances bakes nothing", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  assertEquals(bake_decor_instances(geometry, []), null);
});

Deno.test("bake_decor_instances - one instance's own vertex count matches the source geometry", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: null }];
  const merged = bake_decor_instances(geometry, instances)!;
  assert(merged !== null);
  assertEquals(merged.attributes.position.count, geometry.attributes.position.count);
});

Deno.test("bake_decor_instances - merges N instances into one buffer, N times the source's own vertex count", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [
    { position: [0, 0, 0], rotation_z: 0, scale: 1, id: null },
    { position: [10, 0, 0], rotation_z: 0, scale: 1, id: null },
    { position: [0, 20, 0], rotation_z: 0, scale: 1, id: null },
  ];
  const merged = bake_decor_instances(geometry, instances)!;
  assertEquals(merged.attributes.position.count, geometry.attributes.position.count * instances.length);
});

Deno.test("bake_decor_instances - each instance's own vertices are really translated to its own real position, not just concatenated identically", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const per_instance_vertex_cnt = geometry.attributes.position.count;
  const instances: ModelInstanceTransform[] = [
    { position: [0, 0, 0], rotation_z: 0, scale: 1, id: null },
    { position: [10, 0, 0], rotation_z: 0, scale: 1, id: null },
  ];
  const merged = bake_decor_instances(geometry, instances)!;
  const pos = merged.attributes.position;

  // compose_instance_matrix's own Y-up -> Z-up fix is a fixed rotation about
  // the X axis alone (Y_UP_TO_Z_UP) — a rotation about X never mixes in the
  // X component of a vertex, so the SAME box corner's own X coordinate
  // across two instances differing only in their own X translation must
  // differ by exactly that X offset, regardless of the rotation itself
  // (this is what makes this assertion rotation-agnostic instead of having
  // to hand-derive the full Y/Z remap too).
  for (let vertex = 0; vertex < per_instance_vertex_cnt; vertex++) {
    const x0 = pos.getX(vertex);
    const x1 = pos.getX(per_instance_vertex_cnt + vertex);
    assertAlmostEquals(x1 - x0, 10, 1e-6);
  }
});

Deno.test("bake_decor_instances - scale multiplies the source geometry's own local extent", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [{ position: [0, 0, 0], rotation_z: 0, scale: 2, id: null }];
  const merged = bake_decor_instances(geometry, instances)!;
  merged.computeBoundingBox();
  const size = merged.boundingBox!.getSize(new THREE.Vector3());
  // A unit box scaled by 2 spans 2 real units on every axis (Y-up -> Z-up
  // just permutes which world axis each unit-box axis lands on, not the
  // magnitude), well above the un-scaled 1-unit span.
  assertAlmostEquals(size.length(), new THREE.Vector3(2, 2, 2).length(), 1e-5);
});
