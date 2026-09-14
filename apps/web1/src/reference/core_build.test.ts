// @pxl/web/reference/core_build.test.ts

/**
 * The module's builds against the TypeScript pipeline they replaced: a chunk
 * of a page, with and without placed cubes, through the commands and one
 * tick, must hand over the mesh the TypeScript composite, scan (or direct
 * runs) and bake make — byte for byte, at a fine tier and a coarse one —
 * and a source change, a reclaim and the eraser's hide must do what the
 * node relies on.
 */

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { LodStep } from "@/module/world/lod.ts";
import { cell_index, cell_key, get_objects_in_cell, place, remove, set_virtual_hide } from "@/module/world/object/store.ts";
import { page } from "@/reference/page_fixture.ts";
import { paint_typescript, runs_typescript } from "@/reference/terrain_layer.ts";
import { ObjectWorldLayer } from "@/reference/object_layer.ts";
import { bake_chunk_mesh, BakeContext, type BakedChunkMesh } from "@/reference/chunk_mesher.ts";
import { carve_baked_mesh, carve_visibility, type MeshBlock } from "@/render/mesh/mesh_block.ts";
import { CORE_BUILD, release_chunk_mesh } from "@/wasm/core_build.ts";
import { decor_block_release, type DecorBlock } from "@/render/mesh/decor_block.ts";
import { CORE_POOL, FIXED_ALLOC } from "@/wasm/core_pool.ts";
import { alloc_chunk_volume, release_chunk_volume, seal_chunk_volume } from "@/reference/volume.ts";
import { fill_column_runs, release_column_runs, scan_all_column_runs } from "@/reference/voxel_scan.ts";

const terrain = page(21);
const OFF = ivec.gather(320n, 448n, 0n);
/** Page 0's chunk at OFF: its column's base is its offset. */
const BASE = OFF;

function written_bytes(mesh: BakedChunkMesh): Uint8Array[] {
  const vis = carve_visibility(mesh);
  const block = mesh.block;
  return [
    new Uint8Array(block.buf, block.byte_offset, vis.byteOffset - block.byte_offset),
    new Uint8Array(mesh.index.buffer, mesh.index.byteOffset, mesh.index.byteLength),
  ];
}

function first_difference(a: Uint8Array, b: Uint8Array): number {
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return i;
  return a.length === b.length ? -1 : a.length;
}

/** Places a cube and returns its id. */
function place_cube(x: bigint, y: bigint, z: bigint, color: "red" | "blue"): string {
  const i = ivec.gather(x, y, z);
  place(i, 0n, 1, color);
  const [ix, iy, iz] = cell_index({ i, f: 0n });
  return get_objects_in_cell(cell_key(ix, iy, iz))[0].id;
}

/** The TypeScript pipeline: the direct runs with nothing placed, else the
 * composite and the scan; then the bake. */
function reference(step: LodStep, objects: boolean): BakedChunkMesh {
  let set;
  if (!objects) {
    set = fill_column_runs(64, (w) => runs_typescript(terrain, OFF, false, w));
  } else {
    const v = alloc_chunk_volume();
    paint_typescript(terrain, OFF, false, v);
    new ObjectWorldLayer().paint({ page_idx: 0n, page_off: OFF }, v);
    seal_chunk_volume(v);
    set = scan_all_column_runs(v);
    release_chunk_volume(v);
  }
  const mesh = bake_chunk_mesh(new BakeContext(CORE_POOL, FIXED_ALLOC), set, step);
  release_column_runs(set);
  return mesh;
}

type Heard = { tiers: { generation: u32; step: LodStep; mesh: BakedChunkMesh }[]; runs: u32[] };

function chunk(): { id: u32; heard: Heard } {
  const heard: Heard = { tiers: [], runs: [] };
  const sink = {
    // The module hands the block over; the lanes are carved here, where
    // they are compared.
    on_tier: (generation: u32, step: LodStep, mesh: MeshBlock) => heard.tiers.push({ generation, step, mesh: carve_baked_mesh(mesh.block, mesh.vertex_cnt, mesh.index_cnt) }),
    on_runs: (b: u32) => heard.runs.push(b),
    on_visible: () => {},
    on_layout: () => {},
    on_blend: () => {},
    on_evict: () => {},
    on_decor: (_s: LodStep, d: DecorBlock) => decor_block_release(d),
  };
  const id = CORE_BUILD.add(sink, BASE, 320, 448, 320, 448);
  CORE_BUILD.source(id, terrain, false);
  return { id, heard };
}

function assert_same(actual: BakedChunkMesh, expected: BakedChunkMesh, at: string): void {
  assertEquals([actual.vertex_cnt, actual.index.length], [expected.vertex_cnt, expected.index.length], `${at}: counts`);
  const [a, b] = [written_bytes(actual), written_bytes(expected)];
  assertEquals(first_difference(a[0], b[0]), -1, `${at}: vertex bytes`);
  assertEquals(first_difference(a[1], b[1]), -1, `${at}: index bytes`);
}

Deno.test("core build - a chunk with nothing placed bakes the same tier from its direct runs as from a scan of its volume, at every step", () => {
  // Why the direct path serves every tier of such a chunk (build.rs's
  // advance_runs): a column of terrain alone is one run either way.
  for (const seed of [33, 41, 23]) {
    const page_terrain = page(seed);
    for (const step of [1, 2, 4, 8, 16] as const) {
      const direct = fill_column_runs(64, (w) => runs_typescript(page_terrain, OFF, false, w));
      const v = alloc_chunk_volume();
      paint_typescript(page_terrain, OFF, false, v);
      seal_chunk_volume(v);
      const scanned = scan_all_column_runs(v);
      release_chunk_volume(v);
      assertEquals(direct.count, scanned.count, `seed ${seed}: the runs`);
      const a = bake_chunk_mesh(new BakeContext(CORE_POOL, FIXED_ALLOC), direct, step);
      const b = bake_chunk_mesh(new BakeContext(CORE_POOL, FIXED_ALLOC), scanned, step);
      assert_same(a, b, `seed ${seed} step ${step}`);
      release_chunk_mesh(a);
      release_chunk_mesh(b);
      release_column_runs(direct);
      release_column_runs(scanned);
    }
  }
});

Deno.test("core build - a chunk's tiers are the TypeScript pipeline's meshes, byte for byte, direct and scanned, with cubes placed and without", () => {
  const placed: string[] = [];
  for (const objects of [false, true]) {
    if (objects) { for (let k = 0n; k < 6n; k++) placed.push(place_cube(320n + 5n + k, 448n + 7n, 40n + k, "red")); }
    const { id, heard } = chunk();
    CORE_BUILD.want(id, 8, 1);
    CORE_BUILD.build_tick([id], Infinity);
    assertEquals(heard.tiers.map((t) => t.step), [8, 1], `objects ${objects}: both tiers, in the order asked`);
    assert(heard.runs[0] > 0, "the runs are held and said so");
    // A scan's runs stay for the next tier; direct runs go with the last
    // tier wanted, being cheaper to make again than to keep.
    assertEquals(heard.runs.at(-1)! > 0, objects, `objects ${objects}: the runs kept`);
    for (const t of heard.tiers) {
      assertEquals(t.generation, 1);
      const expected = reference(t.step, objects);
      assert_same(t.mesh, expected, `objects ${objects} step ${t.step}`);
      release_chunk_mesh(expected);
      release_chunk_mesh(t.mesh);
    }
    CORE_BUILD.remove(id);
    CORE_BUILD.pump();
  }
  for (const id of placed) remove(id);
});

Deno.test("core build - a source change mid-build makes what was in flight stale and lets the runs go", () => {
  const { id, heard } = chunk();
  CORE_BUILD.want(id, 1, null);
  // One step: the runs, and the bake opened.
  CORE_BUILD.build_tick([id], 0);
  CORE_BUILD.build_tick([id], 0);
  assertEquals(heard.tiers.length, 0, "a fine bake is not done in two steps");
  CORE_BUILD.source(id, terrain, true);
  CORE_BUILD.want(id, 1, null);
  CORE_BUILD.build_tick([id], Infinity);
  assertEquals(heard.tiers.map((t) => [t.generation, t.step]), [[2, 1]], "only the new generation's tier arrives");
  const mesh = heard.tiers[0].mesh;
  CORE_BUILD.source(id, terrain, true);
  CORE_BUILD.pump();
  assertEquals(heard.runs.at(-1), 0, "a source change lets the runs go");
  release_chunk_mesh(mesh);
  CORE_BUILD.remove(id);
  CORE_BUILD.pump();
});

Deno.test("core build - the eraser's hide leaves its cube out of the drawn build", () => {
  const cube = place_cube(320n + 30n, 448n + 30n, 200n, "blue");
  const build = () => {
    const { id, heard } = chunk();
    CORE_BUILD.want(id, 1, null);
    CORE_BUILD.build_tick([id], Infinity);
    CORE_BUILD.remove(id);
    CORE_BUILD.pump();
    return heard.tiers[0].mesh;
  };
  const shown = build();
  set_virtual_hide(cube);
  const hidden = build();
  set_virtual_hide(null);
  assert(hidden.vertex_cnt < shown.vertex_cnt, `hidden ${hidden.vertex_cnt} vertices, shown ${shown.vertex_cnt}`);
  release_chunk_mesh(shown);
  release_chunk_mesh(hidden);
  remove(cube);
});
