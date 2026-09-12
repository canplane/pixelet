// @pxl/web/render/layer/pipeline/voxel_scan.test.ts

import { assert, assertEquals } from "@std/assert";

import { alloc_chunk_volume, fill_chunk_column, MaterialClass, NO_LAYER, pack_sub_cell_fraction, release_chunk_volume, set_chunk_voxel } from "./volume.ts";
import {
  abort_column_scan,
  begin_column_scan,
  column_scan_open,
  finish_column_scan,
  merge_column_runs,
  release_column_runs,
  run_to_box,
  scan_all_column_runs,
  scan_column_rows,
  topmost_voxel,
} from "./voxel_scan.ts";

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
  const box = run_to_box(4, 5, run);
  assertEquals(half, 128);
  assertEquals(box.min, [4.5, 5.25, 6.5]);
  assertEquals(box.max, [5, 5.75, 7]);
});

const SCAN_LAND = LAND;
const SCAN_ROCK = { material_class: MaterialClass.LAND, r: 90, g: 80, b: 70, layer_idx: NO_LAYER };

/** A chunk with runs in every row, some spanning bands, so a row-stepped
 * scan has something to get wrong at every step boundary. */
function busy_volume() {
  const volume = alloc_chunk_volume();
  for (let cy = 0; cy < 64; cy++) {
    fill_chunk_column(volume, cy, cy, -70, -3 + (cy % 5), SCAN_LAND);
    fill_chunk_column(volume, 63 - cy, cy, 60, 70 + (cy % 3), SCAN_ROCK);
  }
  return volume;
}

Deno.test("column scan - stepping by rows packs exactly what the whole scan packs", () => {
  const whole = busy_volume();
  const expected = scan_all_column_runs(whole);
  release_chunk_volume(whole);

  const stepped_volume = busy_volume();
  const scan = begin_column_scan(stepped_volume);
  assert(scan !== null);
  let steps = 0;
  while (!scan_column_rows(scan, 8)) steps++;
  const actual = finish_column_scan(scan);
  release_chunk_volume(stepped_volume);

  assert(steps >= 7, `expected several steps, got ${steps}`);
  assertEquals(actual.count, expected.count);
  assertEquals(actual.start, expected.start);
  assertEquals(actual.z0, expected.z0);
  assertEquals(actual.z1, expected.z1);
  assertEquals(actual.r, expected.r);
  assertEquals(actual.material_class, expected.material_class);
  release_column_runs(expected);
  release_column_runs(actual);
});

Deno.test("column scan - only one may be open, and aborting frees the slot", () => {
  const a = alloc_chunk_volume();
  const b = alloc_chunk_volume();
  fill_chunk_column(a, 0, 0, 0, 0, SCAN_LAND);
  fill_chunk_column(b, 0, 0, 0, 0, SCAN_LAND);

  const first = begin_column_scan(a);
  assert(first !== null);
  assertEquals(column_scan_open(), true);
  assertEquals(begin_column_scan(b), null);

  abort_column_scan(first);
  assertEquals(column_scan_open(), false);
  const second = begin_column_scan(b);
  assert(second !== null);
  scan_column_rows(second, 64);
  release_column_runs(finish_column_scan(second));
  assertEquals(column_scan_open(), false);

  release_chunk_volume(a);
  release_chunk_volume(b);
});

Deno.test("column scan - each step keeps a run that spans bands whole", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 5, 9, -10, 10, SCAN_LAND); // crosses band -1 / 0
  const scan = begin_column_scan(volume);
  assert(scan !== null);
  while (!scan_column_rows(scan, 1)) { /* one row at a time */ }
  const set = finish_column_scan(scan);
  release_chunk_volume(volume);

  const column = 9 * 64 + 5;
  assertEquals(set.start[column + 1] - set.start[column], 1);
  assertEquals(set.z0[set.start[column]], -10);
  assertEquals(set.z1[set.start[column]], 10);
  release_column_runs(set);
});
