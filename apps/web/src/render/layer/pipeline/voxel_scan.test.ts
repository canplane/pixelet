// @pxl/web/render/layer/pipeline/voxel_scan.test.ts

import { assertEquals } from "@std/assert";

import { alloc_chunk_volume, fill_chunk_column, MaterialClass, NO_LAYER, pack_sub_cell_fraction, set_chunk_voxel } from "./volume.ts";
import { merge_column_runs, run_to_box, topmost_voxel } from "./voxel_scan.ts";

const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;
const OBJECT = { material_class: MaterialClass.OBJECT, r: 40, g: 50, b: 60, layer_idx: NO_LAYER } as const;

Deno.test("voxel scan - a material run stays continuous across a 64-cell band boundary", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 2, 3, 62, 66, LAND);
  const runs = merge_column_runs(volume, 2, 3);
  assertEquals(runs.length, 1);
  assertEquals([runs[0].z0, runs[0].z1], [62, 66]);
});

Deno.test("voxel scan - an optional diagnostic scan can address one dense band", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 2, 3, 62, 66, LAND);
  assertEquals(merge_column_runs(volume, 2, 3, 0).map((run) => [run.z0, run.z1]), [[62, 63]]);
  assertEquals(merge_column_runs(volume, 2, 3, 1).map((run) => [run.z0, run.z1]), [[64, 66]]);
});

Deno.test("voxel scan - terrain higher than the retired 64-cell band is not clipped", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 0, 0, -16, 90, LAND);
  assertEquals(volume.bands.length, 3);
  assertEquals(topmost_voxel(volume, 0, 0)?.z, 90);
  assertEquals([merge_column_runs(volume, 0, 0)[0].z0, merge_column_runs(volume, 0, 0)[0].z1], [-16, 90]);
});

Deno.test("voxel scan - adjacent same-color objects remain distinct runs", () => {
  const volume = alloc_chunk_volume();
  set_chunk_voxel(volume, 1, 1, 2, OBJECT);
  set_chunk_voxel(volume, 1, 1, 3, OBJECT);
  const runs = merge_column_runs(volume, 1, 1);
  assertEquals(runs.map((run) => [run.z0, run.z1]), [[2, 2], [3, 3]]);
});

Deno.test("voxel scan - sub-cell offsets and size decode on an exact 1/256 grid", () => {
  const volume = alloc_chunk_volume();
  const half = pack_sub_cell_fraction(0.5);
  const quarter = pack_sub_cell_fraction(0.25);
  set_chunk_voxel(volume, 4, 5, 6, OBJECT, [half, quarter, half, half]);
  const run = merge_column_runs(volume, 4, 5)[0];
  const box = run_to_box(volume, 4, 5, run);
  assertEquals(half, 128);
  assertEquals(box.min, [4.5, 5.25, 6.5]);
  assertEquals(box.max, [5, 5.75, 7]);
});
