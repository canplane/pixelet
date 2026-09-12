// @pxl/web/module/world/sched/chunk_table.test.ts

import { assert, assertAlmostEquals, assertEquals } from "@std/assert";
import * as THREE from "three";

import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { ChunkTable, EMPTY, IN_FRUSTUM, PENDING, VISIBILITY_CHANGED } from "./chunk_table.ts";

const SPAN = CHUNK_CELL_SPAN * U_PER_CELL;
const base = (x: number, y: number) => ivec.gather(BigInt(x), BigInt(y), 0n);

type Handle = { slot: number; name: string };

function planes_of(frustum: THREE.Frustum): Float64Array {
  const out = new Float64Array(24);
  frustum.planes.forEach((plane, p) => {
    out[p * 4] = plane.normal.x;
    out[p * 4 + 1] = plane.normal.y;
    out[p * 4 + 2] = plane.normal.z;
    out[p * 4 + 3] = plane.constant;
  });
  return out;
}

/** A camera looking down at the origin, its frustum, and the root matrix a
 * rotated and translated frame would hand the table. */
function scene() {
  const camera = new THREE.PerspectiveCamera(50, 16 / 9, 1, 10000);
  camera.position.set(0, -600, 500);
  camera.up.set(0, 0, 1);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld();
  const frustum = new THREE.Frustum().setFromProjectionMatrix(new THREE.Matrix4().multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse));
  const root = new THREE.Matrix4().makeRotationZ(0.3).multiply(new THREE.Matrix4().makeTranslation(-40, 25, 0));
  return { camera, frustum, planes: planes_of(frustum), root };
}

Deno.test("chunk_table - refresh measures what a mounted group's sphere test would", () => {
  const { camera, frustum, planes, root } = scene();
  const table = new ChunkTable<Handle>();
  const origin = { x: 1000, y: -2000 };
  // A ring of chunks around the origin, some in view, some well outside.
  const handles: Handle[] = [];
  for (let gx = -12; gx <= 12; gx += 3) {
    for (let gy = -12; gy <= 12; gy += 3) {
      const h = { slot: 0, name: `${gx},${gy}` };
      table.add(h, base(origin.x + gx * SPAN, origin.y + gy * SPAN));
      handles.push(h);
    }
  }
  table.refresh(root.elements, origin.x, origin.y, 0, planes, camera.position.x, camera.position.y, camera.position.z, 7, 8, 9);

  let seen_in = 0;
  const center = new THREE.Vector3();
  for (const h of handles) {
    const [gx, gy] = h.name.split(",").map(Number);
    center.set(gx * SPAN + SPAN / 2, gy * SPAN + SPAN / 2, 0).applyMatrix4(root);
    const inside = Math.abs(camera.position.x - center.x) <= SPAN / 2 && Math.abs(camera.position.y - center.y) <= SPAN / 2;
    const expect = inside || frustum.intersectsSphere(new THREE.Sphere(center, SPAN * 0.75));
    assertEquals(table.has(h.slot, IN_FRUSTUM), expect, h.name);
    assertAlmostEquals(table.distance2[h.slot], center.distanceToSquared(camera.position), 1e-6);
    assertAlmostEquals(table.lead_distance2[h.slot], center.distanceToSquared(new THREE.Vector3(7, 8, 9)), 1e-6);
    if (expect) seen_in++;
  }
  assert(seen_in > 0 && seen_in < handles.length);
});

Deno.test("chunk_table - a measured box replaces the nominal sphere", () => {
  const { camera, frustum, planes, root } = scene();
  const table = new ChunkTable<Handle>();
  const h = { slot: 0, name: "tall" };
  table.add(h, base(0, 0));
  table.set_bounds(h.slot, 10, 20, 300, 350);
  table.refresh(root.elements, 0, 0, 0, planes, camera.position.x, camera.position.y, camera.position.z, 0, 0, 0);
  const center = new THREE.Vector3(10, 20, 300).applyMatrix4(root);
  assertEquals(table.has(h.slot, IN_FRUSTUM), frustum.intersectsSphere(new THREE.Sphere(center, 350)));
  assertAlmostEquals(table.distance2[h.slot], center.distanceToSquared(camera.position), 1e-6);
});

Deno.test("chunk_table - a visibility flip is flagged once and moves the retained bytes across", () => {
  const { camera, planes, root } = scene();
  const table = new ChunkTable<Handle>();
  const near = { slot: 0, name: "near" };
  const far = { slot: 0, name: "far" };
  table.add(near, base(0, 0));
  table.add(far, base(50 * SPAN, 50 * SPAN));
  table.account(near.slot, 1000);
  table.account(far.slot, 10);
  assertEquals([table.retained_on, table.retained_off], [0, 1010]);

  table.refresh(root.elements, 0, 0, 0, planes, camera.position.x, camera.position.y, camera.position.z, 0, 0, 0);
  assert(table.has(near.slot, IN_FRUSTUM));
  assert(table.has(near.slot, VISIBILITY_CHANGED));
  assert(!table.has(far.slot, IN_FRUSTUM));
  assert(!table.has(far.slot, VISIBILITY_CHANGED));
  assertEquals([table.retained_on, table.retained_off], [1000, 10]);

  table.set_flag(near.slot, VISIBILITY_CHANGED, false);
  table.refresh(root.elements, 0, 0, 0, planes, camera.position.x, camera.position.y, camera.position.z, 0, 0, 0);
  assert(!table.has(near.slot, VISIBILITY_CHANGED));
  assertEquals([table.retained_on, table.retained_off], [1000, 10]);

  // Bytes added while on screen land on that side.
  table.account(near.slot, 5);
  assertEquals([table.retained_on, table.retained_off], [1005, 10]);
});

Deno.test("chunk_table - removing a slot moves the last one into it and tells its handle", () => {
  const table = new ChunkTable<Handle>();
  const a = { slot: 0, name: "a" };
  const b = { slot: 0, name: "b" };
  const c = { slot: 0, name: "c" };
  table.add(a, base(1, 1));
  table.add(b, base(2, 2));
  table.add(c, base(3, 3));
  table.set_flag(c.slot, EMPTY, true);
  table.set_pending(c.slot, true, 77, 1234);
  table.account(b.slot, 40);
  table.account(c.slot, 2);

  table.remove(b.slot);
  assertEquals(table.len, 2);
  assertEquals(c.slot, 1);
  assertEquals(table.handles[1], c);
  assertEquals(table.find(base(3, 3)), 1);
  assertEquals(table.find(base(2, 2)), -1);
  assertEquals(table.base_of(1), base(3, 3));
  assert(table.has(1, EMPTY));
  assert(table.has(1, PENDING));
  assertEquals(table.request[1], 77);
  assertEquals(table.pending_since[1], 1234);
  assertEquals(table.retained[1], 2);
  assertEquals(table.retained_off, 2);
});

Deno.test("chunk_table - the pending clock restarts when the request changes, not when the flag repeats", () => {
  const table = new ChunkTable<Handle>();
  const h = { slot: 0, name: "h" };
  table.add(h, base(0, 0));
  table.set_pending(h.slot, true, 1, 100);
  table.set_pending(h.slot, true, 1, 500);
  assertEquals(table.pending_since[h.slot], 100);
  table.set_pending(h.slot, true, 2, 500);
  assertEquals(table.pending_since[h.slot], 500);
  table.set_pending(h.slot, false, 2, 900);
  assertEquals(table.pending_since[h.slot], -1);
  assert(!table.has(h.slot, PENDING));
});
