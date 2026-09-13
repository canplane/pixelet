// @pxl/web/render/layer/pipeline/wasm_bake.test.ts

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { buddy_alloc } from "@xpute/core/ds/typed_alloc.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

import { LOD_STEP_BY_LEVEL } from "@/render/lod.ts";
import { instantiate_pxl, type PxlExports } from "@/wasm/pxl.ts";
import { MaterialClass, NO_LAYER, SUB_CELL_NONE } from "./voxel.ts";
import { type MeshRun, pack_column_runs, release_column_runs } from "./voxel_scan.ts";
import { bake_chunk_mesh, BakeContext, type BakedChunkMesh, carve_visibility } from "./chunk_mesher.ts";
import { CORE_MEMORY, CORE_POOL, MODULE_SECTION } from "./core_pool.ts";
import { type MeshPool, WasmBaker } from "./wasm_bake.ts";

/** The module over the core memory, which is where pack_column_runs's sets
 * and the mesh pool's blocks are. Instantiated per test: each instance
 * starts the module's statics afresh. */
function pxl(): Promise<PxlExports> {
  assert(CORE_MEMORY !== null, "the core memory reserved");
  return instantiate_pxl(CORE_MEMORY);
}

/** A context whose meshes come from the core pool, as the wasm bake's
 * must, with scratch of its own so the two contexts under test share
 * nothing but the pool. */
function core_context(): BakeContext {
  return new BakeContext(CORE_POOL, buddy_alloc(new BuddyAllocator()));
}

/** A seeded xorshift, so a failure names a scene that can be rebuilt. */
function rng(seed: u32): () => u32 {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}

/**
 * Columns of stacked runs with gaps, objects among the terrain, sub-cell
 * boxes, and layers: every branch the face mask, the merge and the coarse
 * accumulation take. `flat` leaves long equal stretches so the merge has
 * wide rectangles to find rather than single cells.
 *
 * Out of order the way the scan writes them, too: a band's overflow
 * sub-voxels follow its voxel runs (voxel_scan.ts, RunSink), so here a
 * sub-cell box is echoed again a run or two later, below runs already
 * written — the order the neighbour scans' early stops must survive.
 */
function scene(seed: u32, flat: boolean): MeshRun[][] {
  const next = rng(seed);
  const columns: MeshRun[][] = Array.from({ length: CHUNK_CELL_SPAN * CHUNK_CELL_SPAN }, () => []);
  for (let cy = 0; cy < CHUNK_CELL_SPAN; cy++) {
    for (let cx = 0; cx < CHUNK_CELL_SPAN; cx++) {
      const runs = columns[cy * CHUNK_CELL_SPAN + cx];
      const band = flat ? ((cx >> 4) + (cy >> 3)) % 3 : next() % 7;
      let z = -8 + band;
      const stack = 1 + (flat ? (cx % 9 === 0 ? 1 : 0) : next() % 4);
      const overflow: MeshRun[] = [];
      for (let k = 0; k < stack; k++) {
        const kind = flat ? (k === 0 ? 0 : 1) : next() % 6;
        const height = flat ? 3 : next() % 5;
        const colour = flat ? 90 : next() % 256;
        const layer = flat ? 3 : next() % 3 === 0 ? NO_LAYER : next() % 12;
        const run: MeshRun = {
          z0: z,
          z1: z + (kind === 4 ? 0 : height),
          r: colour,
          g: flat ? 140 : next() % 256,
          b: flat ? 60 : next() % 256,
          layer_idx: layer,
          material_class: kind === 1 || kind === 3 ? MaterialClass.OBJECT : MaterialClass.LAND,
          sub_sz: kind === 4 ? 1 + (next() % 255) : SUB_CELL_NONE,
          sub_ox: kind === 4 ? next() % 256 : 0,
          sub_oy: kind === 4 ? next() % 256 : 0,
          sub_oz: kind === 4 ? next() % 256 : 0,
        };
        runs.push(run);
        if (kind === 4 && next() % 2 === 0) overflow.push({ ...run, sub_sz: 1 + (next() % 255), sub_ox: next() % 256 });
        if (overflow.length > 0 && next() % 2 === 0) runs.push(...overflow.splice(0));
        z = run.z1 + 1 + (flat ? 0 : next() % 3);
      }
      runs.push(...overflow);
    }
  }
  return columns;
}

/** Every lane the bake writes, as bytes — the visibility lane is left out:
 * the bake does not write it, and a pool block carries whatever was there. */
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

Deno.test("wasm bake - the Rust mesher bakes the TypeScript mesher's mesh byte for byte, at every tier", async () => {
  const ts_ctx = core_context();
  const wasm_ctx = core_context();
  const baker = new WasmBaker(await pxl(), CORE_POOL, wasm_ctx);

  for (const [seed, flat] of [[1, true], [2, false], [3, false], [0x5eed, false]] as const) {
    const set = pack_column_runs(scene(seed, flat));
    for (const step of LOD_STEP_BY_LEVEL) {
      const expected = bake_chunk_mesh(ts_ctx, set, step);

      const cursor = baker.begin(set, step);
      assert(cursor !== null);
      let steps = 0;
      while (!baker.step(cursor)) steps++;
      const actual = baker.finish(cursor);

      const at = `scene ${seed} at step ${step}`;
      assert(expected.vertex_cnt > 0, `${at} baked nothing, which proves nothing`);
      assert(steps > 2, `${at} took only ${steps} steps`);
      assertEquals(actual.vertex_cnt, expected.vertex_cnt, `${at}: vertex count`);
      assertEquals(actual.index.length, expected.index.length, `${at}: index count`);
      const [a, b] = [written_bytes(actual), written_bytes(expected)];
      for (let k = 0; k < a.length; k++) {
        assertEquals(first_difference(a[k], b[k]), -1, `${at}: ${k === 0 ? "vertex" : "index"} bytes differ`);
      }
      assertEquals(wasm_ctx.occlusion, ts_ctx.occlusion, `${at}: occlusion counters`);
      assertEquals(wasm_ctx.coarse_instances, ts_ctx.coarse_instances, `${at}: coarse counters`);

      CORE_POOL.release(expected.block);
      CORE_POOL.release(actual.block);
    }
    release_column_runs(set);
  }
});

Deno.test("wasm bake - a pool that refuses the mesh starves the bake, which finishes once there is room", async () => {
  // The core pool, refusing until told otherwise: the bake must still get a
  // block in the module's memory once it does.
  let refuse = true;
  const pool: MeshPool = { try_acquire: (bytes) => refuse ? null : CORE_POOL.try_acquire(bytes), release: (block) => CORE_POOL.release(block) };
  const ctx = core_context();
  const baker = new WasmBaker(await pxl(), pool, ctx);
  const set = pack_column_runs(scene(7, false));

  const cursor = baker.begin(set, 8)!;
  while (!baker.step(cursor) && !cursor.starved) { /* until the mesh is asked for */ }
  assert(cursor.starved && !cursor.done);
  const kept = ctx.occlusion.faces_kept;
  assert(!baker.step(cursor) && cursor.starved, "still starved on the next step");
  assertEquals(baker.begin(set, 1), null, "a starved bake still holds the slot");
  refuse = false;
  while (!baker.step(cursor)) assert(!cursor.starved);
  const mesh = baker.finish(cursor);
  assert(ctx.occlusion.faces_kept > kept, "the counters arrive with the mesh");

  const direct = bake_chunk_mesh(core_context(), set, 8);
  assertEquals(mesh.vertex_cnt, direct.vertex_cnt);
  const [a, b] = [written_bytes(mesh), written_bytes(direct)];
  assertEquals(first_difference(a[0], b[0]), -1);
  CORE_POOL.release(mesh.block);
  CORE_POOL.release(direct.block);
  release_column_runs(set);
});

Deno.test("wasm bake - an abort at any point leaves the next bake as it would have been", async () => {
  const ctx = core_context();
  const baker = new WasmBaker(await pxl(), CORE_POOL, ctx);
  const set = pack_column_runs(scene(11, false));

  const bake = (step: 1 | 4): BakedChunkMesh => {
    const c = baker.begin(set, step)!;
    while (!baker.step(c)) { /* whole */ }
    return baker.finish(c);
  };
  const before = [bake(1), bake(4)];
  for (const step of [1, 4] as const) {
    for (const after_steps of [0, 1, 17]) {
      const c = baker.begin(set, step)!;
      for (let i = 0; i < after_steps; i++) baker.step(c);
      baker.abort(c);
      assert(!baker.is_open());
    }
  }
  const after = [bake(1), bake(4)];
  for (let k = 0; k < 2; k++) {
    const [a, b] = [written_bytes(after[k]), written_bytes(before[k])];
    assertEquals(first_difference(a[0], b[0]), -1);
    assertEquals(first_difference(a[1], b[1]), -1);
  }
  for (const m of [...before, ...after]) CORE_POOL.release(m.block);
  release_column_runs(set);
});

Deno.test("wasm bake - the module keeps what the map reserves for it, and its heap stays inside", async () => {
  const exports = await pxl();
  assertEquals(exports.pxl_module_bytes(), MODULE_SECTION.bytes);
  const ctx = core_context();
  const baker = new WasmBaker(exports, CORE_POOL, ctx);
  const set = pack_column_runs(scene(2, false));
  for (const step of [1, 2] as const) {
    const c = baker.begin(set, step)!;
    while (!baker.step(c)) { /* whole */ }
    CORE_POOL.release(baker.finish(c).block);
  }
  assert(baker.heap_used > 0 && baker.heap_used < MODULE_SECTION.bytes / 4, `module heap at ${baker.heap_used} bytes`);
  release_column_runs(set);
});
