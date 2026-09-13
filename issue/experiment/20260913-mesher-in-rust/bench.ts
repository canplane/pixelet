// issue/experiment/20260913-mesher-in-rust/bench.ts
//
// The TypeScript mesher against the Rust one, whole bakes of the same run
// sets at the tiers a view mostly holds, both over the core memory. (When
// this was first run the module had its own memory, and the wasm side
// included a copy in and a copy out; it shares the core memory now and
// copies nothing.)
//
// Usage (from the repo root, after `deno task build:wasm`):
//   deno bench -A --no-check issue/experiment/20260913-mesher-in-rust/bench.ts
// (unchecked: from the root, apps/web's DOM lib does not apply to its files)

import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { buddy_alloc } from "@xpute/core/ds/typed_alloc.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

import { MaterialClass, NO_LAYER, pack_sub_cell, SUB_CELL_NONE, type VoxelMaterial } from "../../../apps/web/src/render/layer/pipeline/voxel.ts";
import { type MeshRun, pack_column_runs, release_column_runs, scan_all_column_runs } from "../../../apps/web/src/render/layer/pipeline/voxel_scan.ts";
import { alloc_chunk_volume, append_chunk_sub_voxel, type ChunkVolume, fill_chunk_column, seal_chunk_volume, set_chunk_voxel } from "../../../apps/web/src/render/layer/pipeline/volume.ts";
import { WasmScanner } from "../../../apps/web/src/render/layer/pipeline/wasm_scan.ts";
import { bake_chunk_mesh, BakeContext } from "../../../apps/web/src/render/layer/pipeline/chunk_mesher.ts";
import { WasmBaker } from "../../../apps/web/src/render/layer/pipeline/wasm_bake.ts";
import { CORE_POOL, CORE_PXL } from "../../../apps/web/src/render/layer/pipeline/core_pool.ts";

// wasm_bake.test.ts's scene: stacked runs with gaps, objects, sub-cells, layers.
function scene(seed: number, flat: boolean): MeshRun[][] {
  let s = seed >>> 0 || 1;
  const next = () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
  const columns: MeshRun[][] = Array.from({ length: CHUNK_CELL_SPAN * CHUNK_CELL_SPAN }, () => []);
  for (let cy = 0; cy < CHUNK_CELL_SPAN; cy++) {
    for (let cx = 0; cx < CHUNK_CELL_SPAN; cx++) {
      const runs = columns[cy * CHUNK_CELL_SPAN + cx];
      let z = -8 + (flat ? ((cx >> 4) + (cy >> 3)) % 3 : next() % 7);
      const stack = 1 + (flat ? (cx % 9 === 0 ? 1 : 0) : next() % 4);
      for (let k = 0; k < stack; k++) {
        const kind = flat ? (k === 0 ? 0 : 1) : next() % 6;
        const height = flat ? 3 : next() % 5;
        const run: MeshRun = {
          z0: z,
          z1: z + (kind === 4 ? 0 : height),
          r: flat ? 90 : next() % 256,
          g: flat ? 140 : next() % 256,
          b: flat ? 60 : next() % 256,
          layer_idx: flat ? 3 : next() % 3 === 0 ? NO_LAYER : next() % 12,
          material_class: kind === 1 || kind === 3 ? MaterialClass.OBJECT : MaterialClass.LAND,
          sub_sz: kind === 4 ? 1 + (next() % 255) : SUB_CELL_NONE,
          sub_ox: kind === 4 ? next() % 256 : 0,
          sub_oy: kind === 4 ? next() % 256 : 0,
          sub_oz: kind === 4 ? next() % 256 : 0,
        };
        runs.push(run);
        z = run.z1 + 1 + (flat ? 0 : next() % 3);
      }
    }
  }
  return columns;
}

const pxl = CORE_PXL!;
const ts_ctx = new BakeContext(CORE_POOL, buddy_alloc(new BuddyAllocator()));
const wasm_ctx = new BakeContext(CORE_POOL, buddy_alloc(new BuddyAllocator()));
const baker = new WasmBaker(pxl, CORE_POOL, wasm_ctx);

for (const [name, set] of [["flat", pack_column_runs(scene(1, true))], ["busy", pack_column_runs(scene(2, false))]] as const) {
  for (const step of [1, 2, 4, 16] as const) {
    const group = `${name} (${set.count} runs) step ${step}`;
    Deno.bench({ name: "ts", group, baseline: true }, () => {
      CORE_POOL.release(bake_chunk_mesh(ts_ctx, set, step).block);
    });
    Deno.bench({ name: "wasm", group }, () => {
      const c = baker.begin(set, step)!;
      while (!baker.step(c)) { /* whole */ }
      CORE_POOL.release(baker.finish(c).block);
    });
  }
}

// The scan, likewise: the TypeScript walk against the Rust one over the
// same composite, both writing their set into the core pool.
{
  const volume = scene_volume();
  Deno.bench({ name: "ts", group: "scan (busy composite)", baseline: true }, () => {
    release_column_runs(scan_all_column_runs(volume));
  });
  const scanner = new WasmScanner(pxl, CORE_POOL);
  Deno.bench({ name: "wasm", group: "scan (busy composite)" }, () => {
    const c = scanner.begin(volume)!;
    while (!scanner.step(c)) { /* whole */ }
    release_column_runs(scanner.finish(c));
  });
}

// wasm_scan.test.ts's composite: stacked runs, band gaps, objects, sub-cells,
// overflow.
function scene_volume(): ChunkVolume {
  let s = 2 >>> 0;
  const next = () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
  const material = (cls: MaterialClass = MaterialClass.LAND): VoxelMaterial => ({
    material_class: cls,
    r: next() % 256,
    g: next() % 256,
    b: next() % 256,
    layer_idx: next() % 3 === 0 ? NO_LAYER : next() % 12,
  });
  const v = alloc_chunk_volume();
  for (let cy = 0; cy < 64; cy++) {
    for (let cx = 0; cx < 64; cx++) {
      const base = -70 + (next() % 20);
      const top = base + 40 + (next() % 60);
      fill_chunk_column(v, cx, cy, base, top, material());
      if (next() % 3 === 0) fill_chunk_column(v, cx, cy, top - 5, top, material());
      if (next() % 7 === 0) fill_chunk_column(v, cx, cy, top + 64 + (next() % 130), top + 200, material());
      if (next() % 5 === 0) {
        const m = material(MaterialClass.OBJECT);
        for (let k = 0; k < 3; k++) set_chunk_voxel(v, cx, cy, top + 1 + k, m);
      }
      if (next() % 6 === 0) {
        const z = top + 2;
        set_chunk_voxel(v, cx, cy, z, material(MaterialClass.OBJECT), [next() % 128, next() % 128, next() % 128, 1 + (next() % 127)]);
        if (next() % 2 === 0) append_chunk_sub_voxel(v, cx, cy, z, material(MaterialClass.OBJECT), pack_sub_cell(next() % 128, next() % 128, next() % 128, 1 + (next() % 127)));
      }
    }
  }
  return seal_chunk_volume(v);
}
