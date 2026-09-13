// @pxl/web/render/layer/terrain/direct_runs.test.ts

/**
 * The direct path for coarse tiers, against the scan path it replaces: the
 * same heightmap, through both, must bake to the same coarse mesh — and the
 * direct path must be the cheaper one by a wide margin, which is the whole
 * reason it exists (issue/20260912-coarse-tiers-what-the-field-does.md).
 */

import { assertEquals } from "@std/assert";

import { alloc_chunk_volume, fill_chunk_column, release_chunk_volume } from "../pipeline/volume.ts";
import { MaterialClass, NO_LAYER } from "../pipeline/voxel.ts";
import { fill_column_runs, release_column_runs, scan_all_column_runs } from "../pipeline/voxel_scan.ts";
import { bake_chunk_mesh } from "../pipeline/chunk_mesher.ts";
import { CORE_BAKE, release_chunk_mesh } from "../pipeline/core_build.ts";

const SPAN = 64;
/** A rolling hillside with a ridge: every column a different height. */
const ele = new Int16Array(SPAN * SPAN);
for (let y = 0; y < SPAN; y++) for (let x = 0; x < SPAN; x++) ele[y * SPAN + x] = 20 + Math.round(8 * Math.sin(x / 5) + 6 * Math.cos(y / 7)) + (x > 40 && x < 44 ? 12 : 0);
const FLOOR = 0;
const material = (i: number) => ({ material_class: MaterialClass.LAND, r: 60 + (i % 7) * 10, g: 120, b: 40, layer_idx: NO_LAYER });

/** The scan path: paint the heightmap into a volume, scan it back. */
function via_scan() {
  const v = alloc_chunk_volume();
  for (let i = 0; i < SPAN * SPAN; i++) fill_chunk_column(v, i % SPAN, Math.floor(i / SPAN), FLOOR, ele[i] - 1, material(i));
  const set = scan_all_column_runs(v);
  release_chunk_volume(v);
  return set;
}

/** The direct path: one run per column, packed. */
function via_direct() {
  return fill_column_runs(SPAN, (w) => {
    for (let i = 0; i < SPAN * SPAN; i++) {
      w.column(i);
      w.run(FLOOR, ele[i] - 1, material(i));
    }
  });
}

Deno.test("direct runs - the same heightmap bakes to the same coarse mesh through both paths", () => {
  for (const step of [8, 16] as const) {
    const a = via_scan(), b = via_direct();
    const ma = bake_chunk_mesh(CORE_BAKE, a, step), mb = bake_chunk_mesh(CORE_BAKE, b, step);
    assertEquals(mb.vertex_cnt, ma.vertex_cnt, `step ${step}: vertex count`);
    assertEquals(Array.from(mb.position), Array.from(ma.position), `step ${step}: positions`);
    assertEquals(Array.from(mb.rgb), Array.from(ma.rgb), `step ${step}: colours`);
    release_chunk_mesh(ma);
    release_chunk_mesh(mb);
    release_column_runs(a);
    release_column_runs(b);
  }
});

Deno.test("direct runs - the direct path is an order of magnitude cheaper than the scan path", () => {
  const time = (f: () => { block: { bytes: number } }, n: number) => {
    // warm
    for (let i = 0; i < 3; i++) release_column_runs(f() as never);
    const t0 = performance.now();
    for (let i = 0; i < n; i++) release_column_runs(f() as never);
    return (performance.now() - t0) / n;
  };
  const scan_ms = time(via_scan, 20);
  const direct_ms = time(via_direct, 20);
  console.log(`   scan ${scan_ms.toFixed(3)} ms/chunk  direct ${direct_ms.toFixed(3)} ms/chunk  ratio ${(scan_ms / direct_ms).toFixed(1)}x`);
  // Measured 10.6x when written and 9.1x to 10.2x across runs on the same
  // machine since; the floor is the order of magnitude, not the last digit.
  assertEquals(direct_ms * 8 < scan_ms, true, `direct ${direct_ms.toFixed(3)} ms is not 8x under scan ${scan_ms.toFixed(3)} ms`);
});
