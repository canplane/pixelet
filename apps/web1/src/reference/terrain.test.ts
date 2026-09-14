// @pxl/web/reference/terrain.test.ts

import { assert, assertEquals } from "@std/assert";

import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { alloc_chunk_volume, release_chunk_volume, seal_chunk_volume } from "@/reference/volume.ts";
import { column_run_set_bytes, type ColumnRunSet } from "@/reference/column_run.ts";
import { fill_column_runs, release_column_runs } from "@/reference/voxel_scan.ts";
import { page } from "@/reference/page_fixture.ts";
import { paint_module, paint_typescript, runs_module, runs_typescript } from "@/reference/terrain_layer.ts";

/** The first index at which two lanes differ, or -1: a diff of a whole
 * band's lanes is too much for the assertion's own printer. */
function first_difference(a: ArrayLike<number>, b: ArrayLike<number>): number {
  if (a.length !== b.length) return Math.min(a.length, b.length);
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return i;
  return -1;
}

const OFFSETS = [[0, 0], [64, 0], [0, 960], [960, 960], [320, 448]].map(([x, y]) => ivec.gather(BigInt(x), BigInt(y), 0n));

Deno.test("terrain - the module paints the chunk the TypeScript adapter paints, cell for cell, at the page's corners and inside", () => {
  const terrain = page(7);
  for (const show_structures of [true, false]) {
    for (const off of OFFSETS) {
      const a = alloc_chunk_volume(), b = alloc_chunk_volume();
      paint_typescript(terrain, off, show_structures, a);
      paint_module(terrain, off, show_structures, b);
      assert(!a.starved && !b.starved);
      seal_chunk_volume(a);
      seal_chunk_volume(b);
      const at = `offset ${ivec.X(off)},${ivec.Y(off)} structures ${show_structures}`;
      assertEquals(b.bands.map((x) => x.band_z), a.bands.map((x) => x.band_z), `${at}: bands`);
      for (let i = 0; i < a.bands.length; i++) {
        assertEquals(first_difference(b.bands[i].material, a.bands[i].material), -1, `${at}: band ${i} materials`);
        assertEquals(first_difference(b.bands[i].layer_idx, a.bands[i].layer_idx), -1, `${at}: band ${i} layers`);
      }
      release_chunk_volume(a);
      release_chunk_volume(b);
    }
  }
});

Deno.test("terrain - the module's direct-path runs are the TypeScript adapter's, byte for byte", () => {
  const terrain = page(11);
  const bytes = (set: ColumnRunSet) => new Uint8Array(set.block.buf, set.block.byte_offset, column_run_set_bytes(set.count, set.span));
  for (const off of OFFSETS) {
    const a = fill_column_runs(64, (w) => runs_typescript(terrain, off, true, w));
    const b = fill_column_runs(64, (w) => runs_module(terrain, off, true, w));
    assertEquals(b.count, a.count);
    assertEquals(first_difference(bytes(b), bytes(a)), -1, `offset ${ivec.X(off)},${ivec.Y(off)}`);
    release_column_runs(a);
    release_column_runs(b);
  }
});
