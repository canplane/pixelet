// @pxl/web/modules/world/decor/decor_hit.test.ts

import { assertEquals } from "@std/assert";
import * as THREE from "three";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { ModelInstanceTransform } from "./instanced_model.ts";
import { decor_field, variant_key } from "./field.ts";
import { raycast_decor } from "./decor_hit.ts";

/** A ray fired straight down through (x, 0, 10) toward -Z — a unit box
 * (BoxGeometry) keeps its own [-0.5, 0.5]^3 bounding cube under
 * compose_instance_matrix's Y-up -> Z-up fix regardless (a 90-degree
 * rotation about X maps an axis-aligned unit cube back onto itself, just
 * relabeling which face is which), so this always physically enters any
 * instance centered at (x, 0, 0). */
function downward_raycaster(x: number): THREE.Raycaster {
  return new THREE.Raycaster(new THREE.Vector3(x, 0, 10), new THREE.Vector3(0, 0, -1));
}

const KEY = variant_key(0, 0);

/** Publishes one chunk's worth of instances and writes them, the way a frame
 * would. Origin zero on both sides, so the field's group stays at the
 * identity these rays are cast in. */
function contribute(instances: ModelInstanceTransform[], base: IVec = 0n, chunk = "test-chunk"): void {
  decor_field.set_chunk(chunk, base, new Map([[KEY, {
    geometry: new THREE.BoxGeometry(1, 1, 1),
    material: new THREE.MeshBasicMaterial(),
    instances,
  }]]));
  decor_field.update(0n);
}

function withdraw(...chunks: string[]): void {
  for (const chunk of chunks.length > 0 ? chunks : ["test-chunk"]) decor_field.clear_chunk(chunk);
  decor_field.update(0n);
}

/** `n` unit boxes two units apart along X, named by their index. */
function row(prefix: string, n: number): ModelInstanceTransform[] {
  return Array.from({ length: n }, (_, i) => ({ position: [i * 2, 0, 0] as const, rotation_z: 0, scale: 1, id: `${prefix}${i}` }));
}

Deno.test("raycast_decor - resolves an instanced hit back to its own user-placed id", () => {
  contribute([{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "leaf-abc" }]);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), "leaf-abc");
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - a procedural (id-less) instance stays transparent to hit-testing", () => {
  contribute([{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: null }]);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), null);
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - resolves to the correct instance among several in one mesh", () => {
  contribute([
    { position: [0, 0, 0], rotation_z: 0, scale: 1, id: "first" },
    { position: [10, 0, 0], rotation_z: 0, scale: 1, id: "second" },
  ]);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), "first");
    assertEquals(raycast_decor(downward_raycaster(10), 100), "second");
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - an instance the eraser hid is withheld from the draw and still hit", () => {
  // The whole reason field.ts keeps a second, invisible mesh: a hover that
  // loses its own target releases the hide, which brings the entry back,
  // which lets the hover find it again.
  contribute([{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "hovered", hidden: true }]);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), "hovered");
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - a chunk's own base offsets every instance it contributes", () => {
  contribute([{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "offset" }], ivec.gather(64n, 0n, 0n));
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), null);
    assertEquals(raycast_decor(downward_raycaster(64), 100), "offset");
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - bounded by max_dist, same as the caller's own already-found hit distance", () => {
  contribute([{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "far-enough" }]);
  try {
    // Ray origin is at z=10, box top face at z=0.5 -> real hit distance ~9.5.
    assertEquals(raycast_decor(downward_raycaster(0), 100), "far-enough");
    assertEquals(raycast_decor(downward_raycaster(0), 1), null);
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - tolerates float noise at the max_dist boundary instead of flickering", () => {
  contribute([{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "on-the-boundary" }]);
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
    withdraw();
  }
});

Deno.test("raycast_decor - nothing contributed, or the ray misses everything, resolves to null", () => {
  assertEquals(raycast_decor(downward_raycaster(0), 100), null);

  contribute([{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "somewhere" }]);
  try {
    assertEquals(raycast_decor(downward_raycaster(1000), 100), null);
  } finally {
    withdraw();
  }
});

Deno.test("decor field - a chunk shrinking leaves nothing behind in its own run", () => {
  // The slots a chunk no longer uses stay allocated to it, so what they hold
  // has to stop existing rather than stay as it was.
  contribute(row("a", 3));
  try {
    assertEquals(raycast_decor(downward_raycaster(4), 100), "a2");
    contribute(row("a", 1));
    assertEquals(raycast_decor(downward_raycaster(0), 100), "a0");
    assertEquals(raycast_decor(downward_raycaster(2), 100), null);
    assertEquals(raycast_decor(downward_raycaster(4), 100), null);
  } finally {
    withdraw();
  }
});

Deno.test("decor field - a chunk outgrowing its run moves without disturbing its neighbour", () => {
  // The run is a power of two from MIN_SLICE, so twelve instances cannot fit
  // where three did: the chunk is handed a different range, and everything
  // else in the buffer has to be untouched by that.
  contribute(row("a", 3), 0n, "chunk-a");
  contribute(row("b", 3), ivec.gather(40n, 0n, 0n), "chunk-b");
  try {
    assertEquals(raycast_decor(downward_raycaster(42), 100), "b1");
    contribute(row("a", 12), 0n, "chunk-a");
    assertEquals(raycast_decor(downward_raycaster(0), 100), "a0");
    assertEquals(raycast_decor(downward_raycaster(22), 100), "a11");
    assertEquals(raycast_decor(downward_raycaster(40), 100), "b0");
    assertEquals(raycast_decor(downward_raycaster(42), 100), "b1");
    assertEquals(raycast_decor(downward_raycaster(44), 100), "b2");
  } finally {
    withdraw("chunk-a", "chunk-b");
  }
});

Deno.test("decor field - withdrawing one chunk frees only its own run", () => {
  contribute(row("a", 3), 0n, "chunk-a");
  contribute(row("b", 3), ivec.gather(40n, 0n, 0n), "chunk-b");
  try {
    withdraw("chunk-a");
    assertEquals(raycast_decor(downward_raycaster(0), 100), null);
    assertEquals(raycast_decor(downward_raycaster(40), 100), "b0");
    assertEquals(raycast_decor(downward_raycaster(44), 100), "b2");
    // The freed range is reusable, and what lands in it is the new chunk's,
    // not a survivor of the old one's.
    contribute(row("c", 2), ivec.gather(80n, 0n, 0n), "chunk-c");
    assertEquals(raycast_decor(downward_raycaster(80), 100), "c0");
    assertEquals(raycast_decor(downward_raycaster(0), 100), null);
    assertEquals(raycast_decor(downward_raycaster(40), 100), "b0");
  } finally {
    withdraw("chunk-b", "chunk-c");
  }
});

Deno.test("decor field - a floating-origin shift rebases every live run", () => {
  contribute(row("a", 2), ivec.gather(40n, 0n, 0n), "chunk-a");
  try {
    assertEquals(raycast_decor(downward_raycaster(40), 100), "a0");
    // The buffers hold origin-relative positions, so moving the origin is the
    // one change that genuinely invalidates all of them.
    decor_field.update(ivec.gather(30n, 0n, 0n));
    assertEquals(raycast_decor(downward_raycaster(10), 100), "a0");
    assertEquals(raycast_decor(downward_raycaster(12), 100), "a1");
    assertEquals(raycast_decor(downward_raycaster(40), 100), null);
  } finally {
    decor_field.clear_chunk("chunk-a");
    decor_field.update(0n);
  }
});

Deno.test("raycast_decor - withdrawing a chunk stops its instances from being hit-tested", () => {
  contribute([{ position: [0, 0, 0], rotation_z: 0, scale: 1, id: "gone-soon" }]);
  assertEquals(raycast_decor(downward_raycaster(0), 100), "gone-soon");
  withdraw();
  assertEquals(raycast_decor(downward_raycaster(0), 100), null);
});
