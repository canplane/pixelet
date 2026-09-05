// @pxl/web/modules/world/decor/decor_hit.test.ts

import { assertEquals } from "@std/assert";
import * as THREE from "three";

import { bake_decor_instances, type ModelInstanceTransform } from "./instanced_model.ts";
import { raycast_decor, register_decor_mesh } from "./decor_hit.ts";

/** A ray fired straight down through (x, 0, 10) toward -Z — a unit box
 * (BoxGeometry) keeps its own [-0.5, 0.5]^3 bounding cube under
 * compose_instance_matrix's Y-up -> Z-up fix regardless (a 90-degree
 * rotation about X maps an axis-aligned unit cube back onto itself, just
 * relabeling which face is which), so this always physically enters any
 * instance centered at (x, 0, 0). */
function downward_raycaster(x: number): THREE.Raycaster {
  return new THREE.Raycaster(new THREE.Vector3(x, 0, 10), new THREE.Vector3(0, 0, -1));
}

Deno.test("raycast_decor - resolves a real mesh hit back to its own user-placed id", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [
    { position: [0, 0, 0], rotation_z: 0, scale: 1, id: "leaf-abc" },
  ];
  const merged = bake_decor_instances(geometry, instances)!;
  const mesh = new THREE.Mesh(merged, new THREE.MeshBasicMaterial());
  const unregister = register_decor_mesh(mesh, instances);
  try {
    const id = raycast_decor(downward_raycaster(0), 100);
    assertEquals(id, "leaf-abc");
  } finally {
    unregister();
  }
});

Deno.test("raycast_decor - a procedural (id-less) instance stays transparent to hit-testing", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [
    { position: [0, 0, 0], rotation_z: 0, scale: 1, id: null },
  ];
  const merged = bake_decor_instances(geometry, instances)!;
  const mesh = new THREE.Mesh(merged, new THREE.MeshBasicMaterial());
  const unregister = register_decor_mesh(mesh, instances);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), null);
  } finally {
    unregister();
  }
});

Deno.test("raycast_decor - resolves to the correct instance among several baked into one mesh", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [
    { position: [0, 0, 0], rotation_z: 0, scale: 1, id: "first" },
    { position: [10, 0, 0], rotation_z: 0, scale: 1, id: "second" },
  ];
  const merged = bake_decor_instances(geometry, instances)!;
  const mesh = new THREE.Mesh(merged, new THREE.MeshBasicMaterial());
  const unregister = register_decor_mesh(mesh, instances);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), "first");
    assertEquals(raycast_decor(downward_raycaster(10), 100), "second");
  } finally {
    unregister();
  }
});

Deno.test("raycast_decor - bounded by max_dist, same as the caller's own already-found hit distance", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "far-enough" }];
  const merged = bake_decor_instances(geometry, instances)!;
  const mesh = new THREE.Mesh(merged, new THREE.MeshBasicMaterial());
  const unregister = register_decor_mesh(mesh, instances);
  try {
    // Ray origin is at z=10, box top face at z=0.5 -> real hit distance ~9.5.
    assertEquals(raycast_decor(downward_raycaster(0), 100), "far-enough");
    assertEquals(raycast_decor(downward_raycaster(0), 1), null);
  } finally {
    unregister();
  }
});

Deno.test("raycast_decor - tolerates float noise at the max_dist boundary instead of flickering", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "on-the-boundary" }];
  const merged = bake_decor_instances(geometry, instances)!;
  const mesh = new THREE.Mesh(merged, new THREE.MeshBasicMaterial());
  const unregister = register_decor_mesh(mesh, instances);
  try {
    // Real hit distance is exactly 9.5 (ray origin z=10, box top face z=0.5).
    // A caller-supplied max_dist a hair under that (as chunk_raycast.ts's own
    // independently-computed AABB distance would be, from pure float64 noise
    // between the two algorithms, never from real occlusion) must still
    // resolve — this is the exact case that used to flip every frame under a
    // real mouse's own sub-pixel jitter (reported 2026-09-05).
    assertEquals(raycast_decor(downward_raycaster(0), 9.5 - 0.0005), "on-the-boundary");
    // A gap an order of magnitude past the tolerance is a real occlusion
    // (something actually closer than this decor mesh), and must still lose —
    // the tolerance must not swallow legitimate max_dist rejections.
    assertEquals(raycast_decor(downward_raycaster(0), 9.5 - 0.01), null);
  } finally {
    unregister();
  }
});

Deno.test("raycast_decor - nothing registered, or the ray misses everything, resolves to null", () => {
  assertEquals(raycast_decor(downward_raycaster(0), 100), null);

  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "somewhere" }];
  const merged = bake_decor_instances(geometry, instances)!;
  const mesh = new THREE.Mesh(merged, new THREE.MeshBasicMaterial());
  const unregister = register_decor_mesh(mesh, instances);
  try {
    assertEquals(raycast_decor(downward_raycaster(1000), 100), null);
  } finally {
    unregister();
  }
});

Deno.test("raycast_decor - unregistering a mesh stops it from being hit-tested", () => {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const instances: ModelInstanceTransform[] = [{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "gone-soon" }];
  const merged = bake_decor_instances(geometry, instances)!;
  const mesh = new THREE.Mesh(merged, new THREE.MeshBasicMaterial());
  const unregister = register_decor_mesh(mesh, instances);
  assertEquals(raycast_decor(downward_raycaster(0), 100), "gone-soon");
  unregister();
  assertEquals(raycast_decor(downward_raycaster(0), 100), null);
});
