// @pxl/web/render/layer/terrain/terrain.test.ts

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { ELE_PADDED_SPAN, PAGE_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import { type Ground, GROUND_CH_KEYS, GroundChannelKey, type Terrain } from "@pxl/shared/bake/format.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { into_core } from "@/module/world/terrain/resident.ts";
import { alloc_chunk_volume, release_chunk_volume, seal_chunk_volume } from "../pipeline/volume.ts";
import { column_run_set_bytes, type ColumnRunSet } from "../pipeline/column_run.ts";
import { fill_column_runs, release_column_runs } from "../pipeline/voxel_scan.ts";
import { GROUND_COLOR_LOOKUP_FLAT } from "./shader_meta.ts";
import { paint_module, paint_typescript, runs_module, runs_typescript } from "./adapter.ts";

/** The first index at which two lanes differ, or -1: a diff of a whole
 * band's lanes is too much for the assertion's own printer. */
function first_difference(a: ArrayLike<number>, b: ArrayLike<number>): number {
  if (a.length !== b.length) return Math.min(a.length, b.length);
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return i;
  return -1;
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

/** The type ids a channel's table knows, so a cell can carry a real one,
 * plus VOID; land also carries SEA, the bare type masked as water. */
function types_of(ch: GroundChannelKey): u32[] {
  const has = GROUND_COLOR_LOOKUP_FLAT[ch]?.has;
  const out: u32[] = [cube_types.common.VOID];
  if (has) { for (let t = 1; t < 256; t++) if (has[t]) out.push(t); }
  return out;
}

/** A page in the core memory: a rolling elevation raster with a few sharp
 * steps, and ground that is mostly land with water, structures and roads
 * sprinkled — enough of each that every channel decides some cell. */
function page(seed: u32): Terrain {
  const next = rng(seed);
  const ele = new Int16Array(ELE_PADDED_SPAN * ELE_PADDED_SPAN);
  for (let i = 0; i < ele.length; i++) ele[i] = -60 + (next() % 300) - (next() % 7 === 0 ? 200 : 0);
  const ground = {} as Ground;
  const types = Object.fromEntries(GROUND_CH_KEYS.map((k) => [k, types_of(k)])) as Record<GroundChannelKey, u32[]>;
  for (const key of GROUND_CH_KEYS) {
    const lane = new Uint8Array(PAGE_CELL_SPAN * PAGE_CELL_SPAN);
    const choices = types[key];
    const sparse = key !== GroundChannelKey.LAND;
    for (let i = 0; i < lane.length; i++) {
      if (sparse && next() % 5 !== 0) continue;
      lane[i] = key === GroundChannelKey.LAND && next() % 9 === 0 ? cube_types.land.SEA : choices[next() % choices.length];
    }
    ground[key] = lane;
  }
  return into_core({ ele, ground }, false);
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
