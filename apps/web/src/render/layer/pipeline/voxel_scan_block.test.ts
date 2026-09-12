// @pxl/web/render/layer/pipeline/voxel_scan_block.test.ts

import { assertEquals, assertThrows } from "@std/assert";

import { alloc_chunk_volume, fill_chunk_column, L2_BAND_CELL_SPAN, MaterialClass, NO_LAYER, release_chunk_volume } from "./volume.ts";
import { begin_column_scan, type ColumnRunSet, finish_column_scan, release_column_runs, scan_all_column_runs, scan_column_rows, scan_next_row, scan_wants_row } from "./voxel_scan.ts";

const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;
const ROCK = { material_class: MaterialClass.LAND, r: 90, g: 80, b: 70, layer_idx: NO_LAYER } as const;
const S = L2_BAND_CELL_SPAN;

/** The runs of one column as plain tuples, for comparing across sets. */
function column(set: ColumnRunSet, cx: number, cy: number): [number, number, number][] {
  const col = cy * set.span + cx;
  const out: [number, number, number][] = [];
  for (let i = set.start[col]; i < set.start[col + 1]; i++) out.push([set.z0[i], set.z1[i], set.r[i]]);
  return out;
}

Deno.test("block scan - a 2x2 unit places each chunk's runs at its column offset, identical to a one-chunk scan", () => {
  // Four chunks with distinguishable content; chunk (1, 0) is empty and skipped.
  const make = (fill: (v: ReturnType<typeof alloc_chunk_volume>) => void) => {
    const v = alloc_chunk_volume();
    fill(v);
    return v;
  };
  const c00 = make((v) => fill_chunk_column(v, 3, 4, 0, 2, LAND));
  const c01 = make((v) => fill_chunk_column(v, 60, 1, 5, 9, ROCK));
  const c11 = make((v) => {
    fill_chunk_column(v, 0, 0, 0, 0, LAND);
    fill_chunk_column(v, 63, 63, 10, 12, ROCK);
  });

  // Reference: each chunk scanned on its own.
  const ref00 = scan_all_column_runs(c00);
  const ref01 = scan_all_column_runs(c01);
  const ref11 = scan_all_column_runs(c11);

  // The block, a chunk-row at a time: [c00, empty] then [c01, c11].
  const scan = begin_column_scan([c00, null], 2 * S)!;
  assertEquals(scan_column_rows(scan, S), false); // first chunk-row walked, unit not done
  assertEquals(scan_wants_row(scan), true);
  scan_next_row(scan, [c01, c11]);
  assertEquals(scan_column_rows(scan, S), true);
  const block = finish_column_scan(scan);

  assertEquals(block.span, 2 * S);
  assertEquals(block.count, ref00.count + ref01.count + ref11.count);
  // Same runs, at the unit's coordinates.
  assertEquals(column(block, 3, 4), column(ref00, 3, 4));
  assertEquals(column(block, 60, S + 1), column(ref01, 60, 1));
  assertEquals(column(block, S + 0, S + 0), column(ref11, 0, 0));
  assertEquals(column(block, S + 63, S + 63), column(ref11, 63, 63));
  // The skipped chunk's columns are empty, and the prefix is monotonic across all of it.
  assertEquals(column(block, S + 5, 5), []);
  for (let c = 1; c <= block.span * block.span; c++) assertEquals(block.start[c] >= block.start[c - 1], true);
  assertEquals(block.start[block.span * block.span], block.count);

  for (const set of [ref00, ref01, ref11, block]) release_column_runs(set);
  for (const v of [c00, c01, c11]) release_chunk_volume(v);
});

Deno.test("block scan - a chunk-row cannot be fed before the current one is walked, nor with the wrong width", () => {
  const v = alloc_chunk_volume();
  const scan = begin_column_scan([v, null], 2 * S)!;
  assertThrows(() => scan_next_row(scan, [null, null]), Error, "before the current chunk-row is walked");
  scan_column_rows(scan, S);
  assertThrows(() => scan_next_row(scan, [null]), Error, "holds 2 chunks");
  scan_next_row(scan, [null, null]);
  assertEquals(scan_column_rows(scan, S), true);
  release_column_runs(finish_column_scan(scan));
  release_chunk_volume(v);
});

Deno.test("block scan - a unit is a whole number of chunks up to the maximum", () => {
  const v = alloc_chunk_volume();
  assertThrows(() => begin_column_scan(v, S + 1), Error, "whole number of chunks");
  assertThrows(() => begin_column_scan(v, 5 * S), Error, "whole number of chunks");
  release_chunk_volume(v);
});
