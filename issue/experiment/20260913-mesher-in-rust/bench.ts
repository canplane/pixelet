// issue/experiment/20260913-mesher-in-rust/bench.ts
//
// The TypeScript mesher against the Rust one, whole bakes of the same run
// sets at the tiers a view mostly holds. The wasm side includes its copy in
// and copy out, since that is what the main thread pays.
//
// Usage (from the repo root, after `deno task build:wasm`):
//   deno bench -A --no-check issue/experiment/20260913-mesher-in-rust/bench.ts
// (unchecked: from the root, apps/web's DOM lib does not apply to its files)

import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { buddy_alloc } from "@xpute/core/ds/typed_alloc.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

import { MaterialClass, NO_LAYER, SUB_CELL_NONE } from "../../../apps/web/src/render/layer/pipeline/voxel.ts";
import { type MeshRun, pack_column_runs } from "../../../apps/web/src/render/layer/pipeline/voxel_scan.ts";
import { bake_chunk_mesh, BakeContext } from "../../../apps/web/src/render/layer/pipeline/chunk_mesher.ts";
import { WasmBaker } from "../../../apps/web/src/render/layer/pipeline/wasm_bake.ts";
import { instantiate_pxl } from "../../../apps/web/src/wasm/pxl.ts";

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

const pxl = await instantiate_pxl();
const ts_ctx = new BakeContext(new BuddyAllocator(), buddy_alloc(new BuddyAllocator()));
const wasm_ctx = new BakeContext(new BuddyAllocator(), buddy_alloc(new BuddyAllocator()));
const baker = new WasmBaker(pxl, wasm_ctx.pool, wasm_ctx);

for (const [name, set] of [["flat", pack_column_runs(scene(1, true))], ["busy", pack_column_runs(scene(2, false))]] as const) {
  for (const step of [1, 2, 4, 16] as const) {
    const group = `${name} (${set.count} runs) step ${step}`;
    Deno.bench({ name: "ts", group, baseline: true }, () => {
      ts_ctx.pool.release(bake_chunk_mesh(ts_ctx, set, step).block);
    });
    Deno.bench({ name: "wasm", group }, () => {
      const c = baker.begin(set, step)!;
      while (!baker.step(c)) { /* whole */ }
      wasm_ctx.pool.release(baker.finish(c).block);
    });
  }
}
