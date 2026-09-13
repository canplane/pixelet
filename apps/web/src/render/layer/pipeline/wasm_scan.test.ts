// @pxl/web/render/layer/pipeline/wasm_scan.test.ts

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";

import type { PxlExports } from "@/wasm/pxl.ts";
import { alloc_chunk_volume, append_chunk_sub_voxel, type ChunkVolume, fill_chunk_column, release_chunk_volume, seal_chunk_volume, set_chunk_voxel } from "./volume.ts";
import { MaterialClass, NO_LAYER, pack_sub_cell, type VoxelMaterial } from "./voxel.ts";
import { column_run_set_bytes, type ColumnRunSet } from "./column_run.ts";
import { release_column_runs, scan_all_column_runs } from "./voxel_scan.ts";
import { CORE_POOL, CORE_PXL } from "./core_pool.ts";
import type { MeshPool } from "./wasm_bake.ts";
import { WasmScanner } from "./wasm_scan.ts";

function pxl(): Promise<PxlExports> {
  assert(CORE_PXL !== null, "the core memory reserved and the module over it");
  return Promise.resolve(CORE_PXL);
}

function rng(seed: u32): () => u32 {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}

function material(next: () => u32, cls: MaterialClass = MaterialClass.LAND): VoxelMaterial {
  return { material_class: cls, r: next() % 256, g: next() % 256, b: next() % 256, layer_idx: next() % 3 === 0 ? NO_LAYER : next() % 12 };
}

/**
 * A composite with everything the walk branches on: columns spanning band
 * boundaries, a gap of whole bands (which breaks a run) and none (which
 * does not), objects stacked on terrain (never merged), sub-cell voxels,
 * overflow sub-voxels sharing a base voxel, and a later write replacing a
 * voxel that had overflow.
 */
function scene(seed: u32): ChunkVolume {
  const next = rng(seed);
  const v = alloc_chunk_volume();
  for (let cy = 0; cy < 64; cy++) {
    for (let cx = 0; cx < 64; cx++) {
      const base = -70 + (next() % 20);
      const top = base + 40 + (next() % 60);
      fill_chunk_column(v, cx, cy, base, top, material(next));
      // A second material part-way up: two runs of the same class touch.
      if (next() % 3 === 0) fill_chunk_column(v, cx, cy, top - 5, top, material(next));
      // Something high above with air (and maybe a whole empty band) between.
      if (next() % 7 === 0) fill_chunk_column(v, cx, cy, top + 64 + (next() % 130), top + 200, material(next));
      // Objects of one material stacked, which still never merge.
      if (next() % 5 === 0) {
        const m = material(next, MaterialClass.OBJECT);
        for (let k = 0; k < 3; k++) set_chunk_voxel(v, cx, cy, top + 1 + k, m);
      }
      // One material ending at a band's top and starting again at a band's
      // bottom: merged across an adjacent band, broken across an empty one.
      // Bands are the chunk's, so the empty ones (4 and 5) must be empty in
      // every column — nothing else here reaches past band 3.
      if (cx % 8 === 0) {
        const m = material(next);
        fill_chunk_column(v, cx, cy, 192, 255, m);
        fill_chunk_column(v, cx, cy, 384, 400, m);
      } else if (cx % 8 === 4) {
        const m = material(next);
        fill_chunk_column(v, cx, cy, -66, -1, m);
        fill_chunk_column(v, cx, cy, 0, 10, m);
      }
      // A sub-cell box, and sometimes overflow sharing its base voxel.
      if (next() % 6 === 0) {
        const z = top + 2;
        set_chunk_voxel(v, cx, cy, z, material(next, MaterialClass.OBJECT), [next() % 128, next() % 128, next() % 128, 1 + (next() % 127)]);
        if (next() % 2 === 0) append_chunk_sub_voxel(v, cx, cy, z, material(next, MaterialClass.OBJECT), pack_sub_cell(next() % 128, next() % 128, next() % 128, 1 + (next() % 127)));
        if (next() % 2 === 0) append_chunk_sub_voxel(v, cx, cy, z, material(next, MaterialClass.OBJECT), pack_sub_cell(0, 0, 0, 64));
        // And sometimes the base voxel is replaced, taking its overflow.
        if (next() % 4 === 0) set_chunk_voxel(v, cx, cy, z, material(next));
      }
    }
  }
  return seal_chunk_volume(v);
}

function bytes_of(set: ColumnRunSet): Uint8Array {
  return new Uint8Array(set.block.buf, set.block.byte_offset, column_run_set_bytes(set.count, set.span));
}

function first_difference(a: Uint8Array, b: Uint8Array): number {
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return i;
  return a.length === b.length ? -1 : a.length;
}

Deno.test("wasm scan - the Rust scan packs the TypeScript scan's run set byte for byte", async () => {
  const scanner = new WasmScanner(await pxl(), CORE_POOL);
  for (const seed of [1, 2, 0x5eed]) {
    const volume = scene(seed);
    const expected = scan_all_column_runs(volume);

    const cursor = scanner.begin(volume);
    assert(cursor !== null);
    let steps = 0;
    while (!scanner.step(cursor)) steps++;
    const actual = scanner.finish(cursor);
    release_chunk_volume(volume);

    assert(expected.count > 4096, `scene ${seed} packed only ${expected.count} runs`);
    assert(steps >= 7, `scene ${seed} took only ${steps} steps`);
    assertEquals(actual.count, expected.count, `scene ${seed}: run count`);
    assertEquals(first_difference(bytes_of(actual), bytes_of(expected)), -1, `scene ${seed}: bytes differ`);
    release_column_runs(expected);
    release_column_runs(actual);
  }
});

Deno.test("wasm scan - a pool that refuses the set starves the scan, which packs once there is room", async () => {
  let refuse = true;
  const pool: MeshPool = { try_acquire: (bytes) => refuse ? null : CORE_POOL.try_acquire(bytes), release: (block) => CORE_POOL.release(block) };
  const scanner = new WasmScanner(await pxl(), pool);
  const volume = scene(3);
  const expected = scan_all_column_runs(volume);

  const cursor = scanner.begin(volume)!;
  while (!scanner.step(cursor) && !cursor.starved) { /* until the set is asked for */ }
  assert(cursor.starved && !cursor.done);
  assert(!scanner.step(cursor) && cursor.starved, "still starved on the next step");
  assertEquals(scanner.begin(volume), null, "a starved scan still holds the slot");
  refuse = false;
  assert(scanner.step(cursor) && !cursor.starved);
  const actual = scanner.finish(cursor);
  release_chunk_volume(volume);
  assertEquals(first_difference(bytes_of(actual), bytes_of(expected)), -1);
  release_column_runs(expected);
  release_column_runs(actual);
});

Deno.test("wasm scan - an abort at any point leaves the next scan as it would have been", async () => {
  const scanner = new WasmScanner(await pxl(), CORE_POOL);
  const volume = scene(4);
  const expected = scan_all_column_runs(volume);
  for (const after_steps of [0, 1, 5]) {
    const c = scanner.begin(volume)!;
    for (let i = 0; i < after_steps; i++) scanner.step(c);
    scanner.abort(c);
    assert(!scanner.is_open());
  }
  const c = scanner.begin(volume)!;
  while (!scanner.step(c)) { /* whole */ }
  const actual = scanner.finish(c);
  release_chunk_volume(volume);
  assertEquals(first_difference(bytes_of(actual), bytes_of(expected)), -1);
  release_column_runs(expected);
  release_column_runs(actual);
});

Deno.test("wasm scan - the module's column walk is merge_column_runs, run for run, at every column", async () => {
  const { column_runs } = await import("./chunk_raycast.ts");
  const { merge_column_runs } = await import("./voxel_scan.ts");
  await pxl();
  const volume = scene(5);
  let runs_seen = 0;
  for (let cy = 0; cy < 64; cy++) {
    for (let cx = 0; cx < 64; cx++) {
      const expected = merge_column_runs(volume, cx, cy);
      const actual = column_runs(volume, cx, cy);
      assertEquals(actual.len, expected.length, `column ${cx},${cy}: run count`);
      for (let i = 0; i < expected.length; i++) {
        const e = expected[i];
        const sub = (e.sub_ox | (e.sub_oy << 8) | (e.sub_oz << 16) | (e.sub_sz << 24)) >>> 0;
        assertEquals([actual.z0[i], actual.z1[i], actual.material_class[i], actual.sub[i]], [e.z0, e.z1, e.material_class, sub], `column ${cx},${cy}: run ${i}`);
      }
      runs_seen += expected.length;
    }
  }
  assert(runs_seen > 4096);
  release_chunk_volume(volume);
});
