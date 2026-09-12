// @pxl/web/module/world/decor/decor_hit.test.ts

import { assertEquals } from "@std/assert";
import * as THREE from "three";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { entry_id_of } from "@/module/world/object/store.ts";

import { decor_field, variant_key } from "./field.ts";
import { HIDDEN_FLAG, instances_push, instances_reset, instances_take, NO_ENTRY } from "./instance.ts";
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

const KEY = variant_key("tree", 0);

/** One instance as the producers push it: a chunk-local position and the
 * store entry it stands for, or `pos: null` for a procedural one. The id a
 * hit resolves to is the store's own derivation from (base, band, pos), so
 * a test names an instance by its pos and asks for the same derivation. */
type Inst = { x: number; pos: number | null; hidden?: boolean };

function id_at(pos: number, base: IVec = 0n): string {
  return entry_id_of(base, 0, pos);
}

/** Publishes one chunk's worth of instances and writes them, the way a frame
 * would. Origin zero on both sides, so the field's group stays at the
 * identity these rays are cast in. */
function contribute(list: Inst[], base: IVec = 0n): void {
  instances_reset();
  for (const inst of list) {
    instances_push(inst.x, 0, 0, 0, 1, inst.hidden ? HIDDEN_FLAG : 0, inst.pos === null ? NO_ENTRY : 0, inst.pos ?? 0);
  }
  const instances = instances_take();
  if (!instances) throw new Error("empty contribution");
  decor_field.set_chunk(
    base,
    new Map([[KEY, {
      geometry: new THREE.BoxGeometry(1, 1, 1),
      material: new THREE.MeshBasicMaterial(),
      instances,
    }]]),
  );
  decor_field.update(0n);
}

function withdraw(...bases: IVec[]): void {
  for (const base of bases.length > 0 ? bases : [0n]) decor_field.clear_chunk(base);
  decor_field.update(0n);
}

/** `n` unit boxes two units apart along X, at pos `first + i`. */
function row(first: number, n: number): Inst[] {
  return Array.from({ length: n }, (_, i) => ({ x: i * 2, pos: first + i }));
}

Deno.test("raycast_decor - resolves an instanced hit back to its own user-placed id", () => {
  contribute([{ x: 0, pos: 7 }]);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), id_at(7));
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - a procedural (id-less) instance stays transparent to hit-testing", () => {
  contribute([{ x: 0, pos: null }]);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), null);
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - resolves to the correct instance among several in one mesh", () => {
  contribute([{ x: 0, pos: 1 }, { x: 10, pos: 2 }]);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), id_at(1));
    assertEquals(raycast_decor(downward_raycaster(10), 100), id_at(2));
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - an instance the eraser hid is withheld from the draw and still hit", () => {
  // The whole reason field.ts keeps a second, invisible mesh: a hover that
  // loses its own target releases the hide, which brings the entry back,
  // which lets the hover find it again.
  contribute([{ x: 0, pos: 7, hidden: true }]);
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), id_at(7));
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - a chunk's own base offsets every instance it contributes", () => {
  contribute([{ x: 0, pos: 7 }], ivec.gather(64n, 0n, 0n));
  try {
    assertEquals(raycast_decor(downward_raycaster(0), 100), null);
    assertEquals(raycast_decor(downward_raycaster(64), 100), id_at(7, ivec.gather(64n, 0n, 0n)));
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - bounded by max_dist, same as the caller's own already-found hit distance", () => {
  contribute([{ x: 0, pos: 7 }]);
  try {
    // Ray origin is at z=10, box top face at z=0.5 -> real hit distance ~9.5.
    assertEquals(raycast_decor(downward_raycaster(0), 100), id_at(7));
    assertEquals(raycast_decor(downward_raycaster(0), 1), null);
  } finally {
    withdraw();
  }
});

Deno.test("raycast_decor - tolerates float noise at the max_dist boundary instead of flickering", () => {
  contribute([{ x: 0, pos: 7 }]);
  try {
    // Real hit distance is exactly 9.5 (ray origin z=10, box top face z=0.5).
    // A caller-supplied max_dist a hair under that (as chunk_raycast.ts's own
    // independently-computed AABB distance would be, from pure float64 noise
    // between the two algorithms, never from real occlusion) must still
    // resolve — this is the exact case that used to flip every frame under a
    // real mouse's own sub-pixel jitter (reported 2026-09-05).
    assertEquals(raycast_decor(downward_raycaster(0), 9.5 - 0.0005), id_at(7));
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

  contribute([{ x: 0, pos: 7 }]);
  try {
    assertEquals(raycast_decor(downward_raycaster(1000), 100), null);
  } finally {
    withdraw();
  }
});

Deno.test("decor field - a chunk shrinking leaves nothing behind in its own run", () => {
  // The slots a chunk no longer uses stay allocated to it, so what they hold
  // has to stop existing rather than stay as it was.
  contribute(row(0, 3));
  try {
    assertEquals(raycast_decor(downward_raycaster(4), 100), id_at(2));
    contribute(row(0, 1));
    assertEquals(raycast_decor(downward_raycaster(0), 100), id_at(0));
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
  contribute(row(0, 3), 0n);
  contribute(row(100, 3), ivec.gather(40n, 0n, 0n));
  try {
    assertEquals(raycast_decor(downward_raycaster(42), 100), id_at(101, ivec.gather(40n, 0n, 0n)));
    contribute(row(0, 12), 0n);
    assertEquals(raycast_decor(downward_raycaster(0), 100), id_at(0));
    assertEquals(raycast_decor(downward_raycaster(22), 100), id_at(11));
    assertEquals(raycast_decor(downward_raycaster(40), 100), id_at(100, ivec.gather(40n, 0n, 0n)));
    assertEquals(raycast_decor(downward_raycaster(42), 100), id_at(101, ivec.gather(40n, 0n, 0n)));
    assertEquals(raycast_decor(downward_raycaster(44), 100), id_at(102, ivec.gather(40n, 0n, 0n)));
  } finally {
    withdraw(0n, ivec.gather(40n, 0n, 0n));
  }
});

Deno.test("decor field - withdrawing one chunk frees only its own run", () => {
  contribute(row(0, 3), 0n);
  contribute(row(100, 3), ivec.gather(40n, 0n, 0n));
  try {
    withdraw(0n);
    assertEquals(raycast_decor(downward_raycaster(0), 100), null);
    assertEquals(raycast_decor(downward_raycaster(40), 100), id_at(100, ivec.gather(40n, 0n, 0n)));
    assertEquals(raycast_decor(downward_raycaster(44), 100), id_at(102, ivec.gather(40n, 0n, 0n)));
    // The freed range is reusable, and what lands in it is the new chunk's,
    // not a survivor of the old one's.
    contribute(row(200, 2), ivec.gather(80n, 0n, 0n));
    assertEquals(raycast_decor(downward_raycaster(80), 100), id_at(200, ivec.gather(80n, 0n, 0n)));
    assertEquals(raycast_decor(downward_raycaster(0), 100), null);
    assertEquals(raycast_decor(downward_raycaster(40), 100), id_at(100, ivec.gather(40n, 0n, 0n)));
  } finally {
    withdraw(ivec.gather(40n, 0n, 0n), ivec.gather(80n, 0n, 0n));
  }
});

Deno.test("decor field - a floating-origin shift rebases every live run", () => {
  contribute(row(0, 2), ivec.gather(40n, 0n, 0n));
  try {
    const base = ivec.gather(40n, 0n, 0n);
    assertEquals(raycast_decor(downward_raycaster(40), 100), id_at(0, base));
    // The buffers hold origin-relative positions, so moving the origin is the
    // one change that genuinely invalidates all of them.
    decor_field.update(ivec.gather(30n, 0n, 0n));
    assertEquals(raycast_decor(downward_raycaster(10), 100), id_at(0, base));
    assertEquals(raycast_decor(downward_raycaster(12), 100), id_at(1, base));
    assertEquals(raycast_decor(downward_raycaster(40), 100), null);
  } finally {
    decor_field.clear_chunk(ivec.gather(40n, 0n, 0n));
    decor_field.update(0n);
  }
});

Deno.test("raycast_decor - withdrawing a chunk stops its instances from being hit-tested", () => {
  contribute([{ x: 0, pos: 7 }]);
  assertEquals(raycast_decor(downward_raycaster(0), 100), id_at(7));
  withdraw();
  assertEquals(raycast_decor(downward_raycaster(0), 100), null);
});
