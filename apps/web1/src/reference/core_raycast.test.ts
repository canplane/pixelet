// @pxl/web/reference/core_raycast.test.ts

/**
 * The module's raycast against the TypeScript one it replaced: the outer
 * chunk walk camera/ray.ts's ray_hit_world made over composites of the
 * real set, and chunk_raycast.ts's ray_hit_chunk inside each — box for
 * box, face for face, over rays from above, from the side, across chunk
 * boundaries, and after a placement, a sub-cell one, and the eraser's hide,
 * which a raycast must see through.
 */

import { assert, assertAlmostEquals, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { type IVec, ivec, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L2_IDX_IN_L3, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";

import { local_world_to_canonical } from "@/module/world/frame/coord.ts";
import { cell_index, cell_key, get_objects_in_cell, place, remove, set_virtual_hide } from "@/module/world/object/store.ts";
import { useTileStore } from "@/module/world/terrain/store.ts";
import { TerrainWorldLayer } from "@/reference/terrain_layer.ts";
import { ObjectWorldLayer } from "@/reference/object_layer.ts";
import { page } from "@/reference/page_fixture.ts";
import { chunk_origin_of } from "@/reference/object_layer.ts";
import { ray_hit_chunk } from "@/reference/chunk_raycast.ts";
import { ray_hit_world, type RayLike, type WorldHit } from "@/module/world/raycast.ts";
import { composite_chunk_layers } from "@/reference/compositor.ts";
import type { ChunkKey } from "@/reference/contract.ts";
import { CORE_BUILD, release_chunk_mesh } from "@/wasm/core_build.ts";
import type { MeshBlock } from "@/render/mesh/mesh_block.ts";
import { decor_block_release, type DecorBlock } from "@/render/mesh/decor_block.ts";
import { MaterialClass } from "@/render/mesh/voxel.ts";
import { release_chunk_volume } from "@/reference/volume.ts";

/** The stack the client composited before the module did: terrain, then
 * the objects over it. */
const LAYERS = [new TerrainWorldLayer(), new ObjectWorldLayer()];

const PAGE_IDX = ivec.gather(0n, 0n, 0n);
const terrain = page(23);
useTileStore.getState().set_tile(PAGE_IDX, { terrain });

const CHUNK_W = CHUNK_CELL_SPAN * U_PER_CELL;

/** The module holds the four chunks around (384, 512). */
const BASES: [number, number][] = [[320, 448], [384, 448], [320, 512], [384, 512]];
const ids: u32[] = [];
for (const [bx, by] of BASES) {
  const sink = {
    on_tier: (_g: u32, _s: number, m: MeshBlock) => release_chunk_mesh(m),
    on_runs: () => {},
    on_visible: () => {},
    on_layout: () => {},
    on_blend: () => {},
    on_evict: () => {},
    on_decor: (_s: number, d: DecorBlock) => decor_block_release(d),
  };
  const id = CORE_BUILD.add(sink, ivec.gather(BigInt(bx), BigInt(by), 0n), bx, by, bx, by, 0);
  CORE_BUILD.source(id, terrain, false);
  ids.push(id);
}
CORE_BUILD.pump();

/** camera/ray.ts's outer chunk DDA as it was, over TypeScript composites
 * of the real set made fresh for every chunk — of the chunks the module
 * holds, as it walks only those. */
function reference(ray: RayLike, map_origin: IVec, max_dist: number): WorldHit | null {
  const ox = ray.origin.x, oy = ray.origin.y, dx = ray.direction.x, dy = ray.direction.y;
  const test_chunk = (gx: number, gy: number): WorldHit | null => {
    const abs: PVF3 = local_world_to_canonical({ i: 0n, f: 0n }, [(gx + 0.5) * CHUNK_W, (gy + 0.5) * CHUNK_W, 0], map_origin);
    const key: ChunkKey = { page_idx: L3_IDX(abs.i), page_off: L2_IDX_IN_L3(abs.i) };
    const base = chunk_origin_of(key.page_idx, key.page_off);
    if (!BASES.some(([bx, by]) => BigInt(bx) === ivec.X(base) && BigInt(by) === ivec.Y(base))) return null;
    const volume = composite_chunk_layers(LAYERS, key, { for_raycast: true });
    if (!volume) return null;
    try {
      const delta = ivec.sub(chunk_origin_of(key.page_idx, key.page_off), map_origin);
      return ray_hit_chunk(ray, volume, ivec.to_real(ivec.X(delta)), ivec.to_real(ivec.Y(delta)), max_dist);
    } finally {
      release_chunk_volume(volume);
    }
  };
  if (Math.abs(dx) < 1e-9 && Math.abs(dy) < 1e-9) return test_chunk(Math.floor(ox / CHUNK_W), Math.floor(oy / CHUNK_W));
  let gx = Math.floor(ox / CHUNK_W), gy = Math.floor(oy / CHUNK_W);
  const step_x = dx > 0 ? 1 : -1, step_y = dy > 0 ? 1 : -1;
  const t_delta_x = Math.abs(dx) < 1e-9 ? Infinity : CHUNK_W / Math.abs(dx);
  const t_delta_y = Math.abs(dy) < 1e-9 ? Infinity : CHUNK_W / Math.abs(dy);
  let t_max_x = Math.abs(dx) < 1e-9 ? Infinity : ((dx > 0 ? gx + 1 : gx) * CHUNK_W - ox) / dx;
  let t_max_y = Math.abs(dy) < 1e-9 ? Infinity : ((dy > 0 ? gy + 1 : gy) * CHUNK_W - oy) / dy;
  const max_chunks = Math.ceil(max_dist / CHUNK_W) + 2;
  for (let i = 0; i < max_chunks; i++) {
    const found = test_chunk(gx, gy);
    if (found) return found;
    if (Math.min(t_max_x, t_max_y) > max_dist) break;
    if (t_max_x < t_max_y) {
      gx += step_x;
      t_max_x += t_delta_x;
    } else {
      gy += step_y;
      t_max_y += t_delta_y;
    }
  }
  return null;
}

function ray(o: [number, number, number], d: [number, number, number]): RayLike {
  const n = Math.hypot(...d);
  return { origin: { x: o[0], y: o[1], z: o[2] }, direction: { x: d[0] / n, y: d[1] / n, z: d[2] / n } };
}

function assert_same(actual: WorldHit | null, expected: WorldHit | null, at: string): void {
  if (expected === null || actual === null) {
    assertEquals(actual, expected, `${at}: one side missed`);
    return;
  }
  assertEquals(actual.material_class, expected.material_class, `${at}: class`);
  assertEquals(actual.face_normal, expected.face_normal, `${at}: normal`);
  assertEquals(actual.box_min, expected.box_min, `${at}: box min`);
  assertEquals(actual.box_max, expected.box_max, `${at}: box max`);
  assertAlmostEquals(actual.t, expected.t, 1e-9, `${at}: t`);
  for (let k = 0; k < 3; k++) assertAlmostEquals(actual.hit_point[k], expected.hit_point[k], 1e-9, `${at}: point`);
}

function placed_id(x: bigint, y: bigint, z: bigint): string {
  const [ix, iy, iz] = cell_index({ i: ivec.gather(x, y, z), f: 0n });
  return get_objects_in_cell(cell_key(ix, iy, iz))[0].id;
}

/** Terrain's top in the fixture, at a page cell. */
function top_at(x: number, y: number): number {
  const hit = ray_hit_world(ray([x + 0.5, y + 0.5, 1000], [0, 0, -1]), 0n, 2000);
  assert(hit !== null, `terrain under (${x}, ${y})`);
  return hit.box_max[2];
}

Deno.test("core raycast - rays from above, from the side and across chunk boundaries strike the boxes the TypeScript walk struck", () => {
  let seed = 5;
  const next = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
  const origins: IVec[] = [0n, ivec.gather(256n, 384n, 0n)];
  let hits = 0;
  for (const map_origin of origins) {
    const ox0 = ivec.to_real(ivec.X(map_origin)), oy0 = ivec.to_real(ivec.Y(map_origin));
    for (let k = 0; k < 60; k++) {
      // Aimed at a point in the four chunks from around and above it.
      const tx = 320 + next() * 128 - ox0, ty = 448 + next() * 128 - oy0;
      const from: [number, number, number] = [tx + (next() * 2 - 1) * 90, ty + (next() * 2 - 1) * 90, 200 + next() * 300];
      const r = ray(from, [tx - from[0], ty - from[1], -from[2] + (next() * 2 - 1) * 40]);
      const max_dist = 100 + next() * 500;
      const expected = reference(r, map_origin, max_dist);
      const actual = ray_hit_world(r, map_origin, max_dist);
      assert_same(actual, expected, `origin ${map_origin} ray ${k}`);
      if (expected) hits++;
    }
    // Straight down, and a ray with one axis flat.
    assert_same(ray_hit_world(ray([352.5 - ox0, 480.5 - oy0, 500], [0, 0, -1]), map_origin, 1000), reference(ray([352.5 - ox0, 480.5 - oy0, 500], [0, 0, -1]), map_origin, 1000), "down");
    assert_same(ray_hit_world(ray([300 - ox0, 470.3 - oy0, 40], [1, 0, -0.2]), map_origin, 400), reference(ray([300 - ox0, 470.3 - oy0, 40], [1, 0, -0.2]), map_origin, 400), "flat y");
  }
  assert(hits > 40, `most rays land: ${hits}`);
});

Deno.test("core raycast - a placement is struck after its chunk is told, a sub-cell box by its inset, and the eraser's hide is seen through", () => {
  const [x, y] = [400, 470];
  const id = ids[BASES.findIndex(([bx, by]) => bx === 384 && by === 448)];
  const top = top_at(x, y);
  const down = ray([x + 0.5, y + 0.5, top + 50], [0, 0, -1]);
  const before = ray_hit_world(down, 0n, 100)!;
  assertEquals(before.material_class, MaterialClass.LAND);

  place(ivec.gather(BigInt(x), BigInt(y), BigInt(top)), 0n, 1, "red");
  const cube = placed_id(BigInt(x), BigInt(y), BigInt(top));
  // The chunk's sources changed: what the node tells the module.
  CORE_BUILD.source(id, terrain, false);
  const struck = ray_hit_world(down, 0n, 100);
  assert_same(struck, reference(down, 0n, 100), "cube");
  assertEquals(struck!.material_class, MaterialClass.OBJECT);
  assertEquals(struck!.box_max[2], top + 1);

  // Hidden for the eraser's preview, still struck.
  set_virtual_hide(cube);
  assert_same(ray_hit_world(down, 0n, 100), struck, "hidden");
  set_virtual_hide(null);

  // A quarter cube on top of it, struck by its own inset box.
  place(ivec.gather(BigInt(x), BigInt(y), BigInt(top + 1)), 0n, 0.25, "blue");
  const small = placed_id(BigInt(x), BigInt(y), BigInt(top + 1));
  CORE_BUILD.source(id, terrain, false);
  const on_small = ray([x + 0.125, y + 0.125, top + 50], [0, 0, -1]);
  const quarter = ray_hit_world(on_small, 0n, 100);
  assert_same(quarter, reference(on_small, 0n, 100), "quarter");
  assertAlmostEquals(quarter!.box_max[0] - quarter!.box_min[0], 0.25, 1e-2);

  remove(small);
  remove(cube);
  CORE_BUILD.source(id, terrain, false);
  assert_same(ray_hit_world(down, 0n, 100), before, "removed");
});

Deno.test("core raycast - the chunks are given back", () => {
  for (const id of ids) CORE_BUILD.remove(id);
  CORE_BUILD.pump();
  assertEquals(ray_hit_world(ray([352.5, 480.5, 500], [0, 0, -1]), 0n, 1000), null);
});
