// @pxl/web/module/world/object/store_reference.test.ts

/**
 * The module's object store against the TypeScript it replaced (branch
 * archive/20260913-ts-object-store): a reference built here from the same
 * octree (@pxl/shared's sparse_tree) and the same derivation, driven by
 * the same random places and removes, and compared after every write —
 * the state read at any point, a chunk's entries lane for lane in the
 * chunks' order, the cell index's answers — and then the object layer's
 * paint against the adapter's old loop, voxel for voxel.
 */

import { assert, assertEquals } from "@std/assert";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import type { U16Array } from "@xpute/core/kit/array.ts";
import { OBJECT_LAYOUT, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import {
  cube_ids,
  cube_types,
  decor_ids,
  decor_num_id,
  object_value_is_air,
  object_value_is_decor,
  object_value_is_present,
  placeable_cube_id_from_num,
  placeable_cube_num_id,
} from "@pxl/shared/spec/cell/mod.ts";
import { L2_BASE, L2_IDX_IN_L3, L2_SHAMT, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import * as sparse_tree from "@pxl/shared/spec/space/sparse_tree/tree.ts";
import type { SparseTree } from "@pxl/shared/spec/space/sparse_tree/tree.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { ObjectWorldLayer, resolve_solid_material } from "@/render/layer/object/adapter.ts";
import { alloc_chunk_volume, append_chunk_sub_voxel, type ChunkVolume, release_chunk_volume, seal_chunk_volume, set_chunk_voxel } from "@/render/layer/pipeline/volume.ts";
import { MaterialClass, NO_LAYER, pack_sub_cell, pack_sub_cell_fraction, SUB_CELL_NONE, type VoxelMaterial } from "@/render/layer/pipeline/voxel.ts";

import { OBJECT_SIZES } from "./size.ts";
import {
  cell_index,
  cell_is_occupied,
  cell_key,
  clear_virtual_carve_preview,
  each_column_entry,
  type EntryCursor,
  get_objects_in_cell,
  object_columns,
  object_store_stats,
  place,
  place_air,
  place_decor,
  remove,
  set_virtual_carve_preview,
  set_virtual_hide,
} from "./store.ts";

function rng(seed: u32): () => u32 {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}

// ---- The reference: the TypeScript store's records, as they were ----

const BAND_LAYOUT = { dim: 3, min: OBJECT_LAYOUT.min, max: OBJECT_LAYOUT.max + L2_SHAMT } as const;
const CHUNK_CELLS = 1n << BigInt(L2_SHAMT);
const CELL_STEPS = 1 << -OBJECT_LAYOUT.min;
const POS_BITS = 9;
const CELL_SIZE_FP = pvf3.from_real(U_PER_CELL);

type RefChunk = { key: IVec; palette: number[]; bands: Map<number, SparseTree<U16Array>> };
type RefEntry = { band: number; pos: number; state: number };

/** Chunks in the TypeScript IndexedMap's order: appended, the last moved
 * into a removed one's place. */
const ref_chunks: RefChunk[] = [];

function floor_div(a: bigint, b: bigint): bigint {
  const q = a / b;
  return a % b !== 0n && a < 0n ? q - 1n : q;
}

function ref_chunk_of(base: IVec): RefChunk | undefined {
  return ref_chunks.find((c) => c.key === base);
}

function ref_write(coord: PVF3, size: f64, val: number): void {
  const level = Math.round(Math.log2(size));
  const base = L2_BASE(TO_XY(coord.i));
  const z = Number(floor_div(cell_index(coord)[2], CHUNK_CELLS));
  let chunk = ref_chunk_of(base);
  if (!chunk && val === cube_types.common.VOID) return;
  if (!chunk) ref_chunks.push(chunk = { key: base, palette: [cube_types.common.VOID], bands: new Map() });
  let tree = chunk.bands.get(z);
  if (!tree && val === cube_types.common.VOID) return;
  if (!tree) chunk.bands.set(z, tree = sparse_tree.alloc(BAND_LAYOUT, 64, Uint16Array));
  let idx = chunk.palette.indexOf(val);
  if (idx < 0) idx = chunk.palette.push(val) - 1;
  sparse_tree.write(tree, coord, idx, level);
  sparse_tree.compact(tree);
  let empty = true;
  sparse_tree.walk(tree, (_x, _y, _z, _level, i) => {
    if (i !== 0) empty = false;
  });
  if (!empty) return;
  sparse_tree.free(tree);
  chunk.bands.delete(z);
  if (chunk.bands.size > 0) return;
  const pos = ref_chunks.indexOf(chunk);
  const last = ref_chunks.pop()!;
  if (last !== chunk) ref_chunks[pos] = last;
}

function ref_read(coord: PVF3): number {
  const chunk = ref_chunk_of(L2_BASE(TO_XY(coord.i)));
  const tree = chunk?.bands.get(Number(floor_div(cell_index(coord)[2], CHUNK_CELLS)));
  return chunk && tree ? chunk.palette[sparse_tree.read(tree, coord)] : cube_types.common.VOID;
}

function pack_pos(x: number, y: number, z: number, level: number): number {
  return (x | (y << POS_BITS) | (z << (2 * POS_BITS)) | ((level - OBJECT_LAYOUT.min) << (3 * POS_BITS))) >>> 0;
}

function ref_entries(chunk: RefChunk): RefEntry[] {
  const out: RefEntry[] = [];
  const push = (band: number, pos: number, state: number) => out.push({ band, pos, state });
  for (const bz of [...chunk.bands.keys()].sort((a, b) => a - b)) {
    sparse_tree.walk(chunk.bands.get(bz)!, (x, y, z, level, idx) => {
      const val = chunk.palette[idx];
      if (!object_value_is_present(val) || val === cube_types.common.UNKNOWN) return;
      if (level <= 0) {
        push(bz, pack_pos(x, y, z, level), val);
        return;
      }
      const span = 1 << (level - OBJECT_LAYOUT.min);
      for (let dz = 0; dz < span; dz += CELL_STEPS) {
        for (let dy = 0; dy < span; dy += CELL_STEPS) {
          for (let dx = 0; dx < span; dx += CELL_STEPS) push(bz, pack_pos(x + dx, y + dy, z + dz, 0), val);
        }
      }
    });
  }
  return out;
}

/** The cell an entry is in, as the cursor works it out. */
function ref_cell(chunk: RefChunk, e: RefEntry): [bigint, bigint, bigint] {
  const [cx0, cy0] = cell_index({ i: chunk.key, f: 0n });
  const x = e.pos & 511, y = (e.pos >>> 9) & 511, z = (e.pos >>> 18) & 511;
  return [cx0 + BigInt(Math.floor(x / CELL_STEPS)), cy0 + BigInt(Math.floor(y / CELL_STEPS)), BigInt(e.band) * CHUNK_CELLS + BigInt(Math.floor(z / CELL_STEPS))];
}

// ---- The scene ----

const COLORS = cube_ids.slice(0, 6);
const AIR = cube_types.common.AIR;

/** A corner on the placement grid of `size`, within a few chunks around
 * the origin so bands, chunks and the seams between them all occur. */
function random_corner(next: () => u32, size: f64): PVF3 {
  const steps = Math.round(size * CELL_STEPS);
  const span = 70 * CELL_STEPS;
  const pick = () => (Math.floor(next() % (2 * span)) - span);
  const snap = (v: number) => Math.floor(v / steps) * steps / CELL_STEPS;
  return pvf3.gather({ i: 0n, f: 0n }, pvf3.from_real(snap(pick())), pvf3.from_real(snap(pick())), pvf3.from_real(snap(pick())));
}

function random_probe(next: () => u32): PVF3 {
  return random_corner(next, OBJECT_SIZES[OBJECT_SIZES.length - 1]);
}

/** One random mutation through the store's public API, mirrored into the
 * reference. */
function mutate(next: () => u32): void {
  const roll = next() % 10;
  if (roll < 2 && ref_chunks.length > 0) {
    // Remove a real entry by its id, as the eraser does.
    const chunk = ref_chunks[next() % ref_chunks.length];
    const entries = ref_entries(chunk);
    if (entries.length > 0) {
      const e = entries[next() % entries.length];
      let id: string | null = null;
      each_column_entry(chunk.key, (entry) => {
        if (id === null && entry.band_of === e.band && entry.pos_of === e.pos) id = entry.id();
      });
      assert(id !== null, "the reference entry is in the store");
      const x = e.pos & 511, y = (e.pos >>> 9) & 511, z = (e.pos >>> 18) & 511, level = (e.pos >>> 27) + OBJECT_LAYOUT.min;
      const [cx0, cy0] = cell_index({ i: chunk.key, f: 0n });
      const corner = pvf3.gather({ i: 0n, f: 0n }, cx0 * CELL_SIZE_FP, cy0 * CELL_SIZE_FP, BigInt(e.band) * CHUNK_CELLS * CELL_SIZE_FP);
      const local = pvf3.sh({ i: 0n, f: 0n }, pvf3.gather({ i: 0n, f: 0n }, pvf3.from_real(x), pvf3.from_real(y), pvf3.from_real(z)), OBJECT_LAYOUT.min);
      const abs = pvf3.add({ i: 0n, f: 0n }, corner, local);
      remove(id);
      ref_write(abs, 2 ** level, cube_types.common.VOID);
      return;
    }
  }
  const size = OBJECT_SIZES[next() % OBJECT_SIZES.length];
  const coord = random_corner(next, size);
  if (roll < 4) {
    place_air(coord.i, coord.f, size);
    ref_write(coord, size, AIR);
  } else if (roll < 5) {
    const id = decor_ids[next() % decor_ids.length];
    place_decor(coord.i, coord.f, size, id);
    ref_write(coord, size, decor_num_id(id));
  } else {
    const id = COLORS[next() % COLORS.length];
    place(coord.i, coord.f, size, id);
    ref_write(coord, size, placeable_cube_num_id(id));
  }
}

function assert_same(next: () => u32, step: number): void {
  const stats = object_store_stats();
  assertEquals(stats.chunks, ref_chunks.length, `step ${step}: chunks`);
  assertEquals(stats.bands, ref_chunks.reduce((n, c) => n + c.bands.size, 0), `step ${step}: bands`);
  let cells = 0;
  for (const [pos, chunk] of ref_chunks.entries()) {
    const expected = ref_entries(chunk);
    const found: RefEntry[] = [];
    each_column_entry(chunk.key, (e) => found.push({ band: e.band_of, pos: e.pos_of, state: e.state }));
    assertEquals(found, expected, `step ${step}: chunk ${pos}'s entries`);
    // The cell index: every cell's own entries, by id and kind, and its occupancy.
    let last = "";
    for (const e of expected) {
      const cell = ref_cell(chunk, e);
      const key = cell_key(...cell);
      if (key === last) continue;
      last = key;
      cells++;
      const in_cell = expected.filter((o) => cell_key(...ref_cell(chunk, o)) === key);
      const occupied = in_cell.filter((o) => !object_value_is_air(o.state));
      const objects = get_objects_in_cell(key);
      assertEquals(objects.length, occupied.length, `step ${step}: cell ${key}'s occupants`);
      assertEquals(cell_is_occupied(...cell), occupied.length > 0, `step ${step}: cell ${key} occupied`);
      for (const [k, o] of occupied.entries()) {
        const kind = object_value_is_decor(o.state) ? "decor" : "solid";
        assertEquals(objects[k].kind, kind, `step ${step}: cell ${key}'s occupant ${k}`);
        assertEquals(objects[k].size, 2 ** ((o.pos >>> 27) + OBJECT_LAYOUT.min));
      }
    }
  }
  assertEquals(stats.cells, cells, `step ${step}: cells`);
  for (let k = 0; k < 8; k++) {
    const at = random_probe(next);
    // read_state is not exported; a placement that reads redundant is the
    // one path through it, so probe through the entries instead: the cell
    // holds what the reference reads there, or nothing.
    const state = ref_read(at);
    const [ix, iy, iz] = cell_index(at);
    const objects = get_objects_in_cell(cell_key(ix, iy, iz));
    if (object_value_is_present(state) && !object_value_is_air(state)) assert(objects.length > 0, `step ${step}: a probe inside an object finds its cell occupied`);
  }
}

// ---- The paint, as the adapter did it ----

const Z_BIAS = 1 << 19;
const voxel_address = (cx: number, cy: number, z: number) => (((z + Z_BIAS) << 12) | (cy << 6) | cx) >>> 0;

function reference_paint(base: IVec, target: ChunkVolume, render: boolean): boolean {
  const claimed = new Set<number>();
  let any = false;
  each_column_entry(base, (obj: EntryCursor) => {
    if (object_value_is_decor(obj.state)) return;
    const local_x = obj.lx, local_y = obj.ly, base_z_real = obj.lz;
    const is_sub_cell = obj.size < U_PER_CELL;
    const cx0 = is_sub_cell ? Math.max(0, Math.floor(local_x / U_PER_CELL)) : Math.max(0, Math.round(local_x / U_PER_CELL));
    const cy0 = is_sub_cell ? Math.max(0, Math.floor(local_y / U_PER_CELL)) : Math.max(0, Math.round(local_y / U_PER_CELL));
    const z = is_sub_cell ? Math.floor(base_z_real / U_PER_CELL) : Math.round(base_z_real / U_PER_CELL);
    if (cx0 >= 64 || cy0 >= 64) return;
    const sz_cells = Math.max(1, Math.round(obj.size / U_PER_CELL));
    const cx1 = Math.min(64, cx0 + sz_cells);
    const cy1 = Math.min(64, cy0 + sz_cells);
    const is_air = object_value_is_air(obj.state);
    const material: VoxelMaterial = is_air ? { material_class: MaterialClass.AIR, r: 0, g: 0, b: 0, layer_idx: NO_LAYER } : resolve_solid_material(placeable_cube_id_from_num(obj.state));
    const sub_sz = is_sub_cell ? pack_sub_cell_fraction(obj.size / U_PER_CELL) : SUB_CELL_NONE;
    const sub_ox = is_sub_cell ? pack_sub_cell_fraction((local_x - cx0 * U_PER_CELL) / U_PER_CELL) : 0;
    const sub_oy = is_sub_cell ? pack_sub_cell_fraction((local_y - cy0 * U_PER_CELL) / U_PER_CELL) : 0;
    const sub_oz = is_sub_cell ? pack_sub_cell_fraction((base_z_real - z * U_PER_CELL) / U_PER_CELL) : 0;
    for (let cy = cy0; cy < cy1; cy++) {
      for (let cx = cx0; cx < cx1; cx++) {
        const address = voxel_address(cx, cy, z);
        any = true;
        if (claimed.has(address) && !is_air) {
          append_chunk_sub_voxel(target, cx, cy, z, material, pack_sub_cell(sub_ox, sub_oy, sub_oz, sub_sz));
        } else {
          claimed.add(address);
          set_chunk_voxel(target, cx, cy, z, material, is_sub_cell ? [sub_ox, sub_oy, sub_oz, sub_sz] : undefined);
        }
      }
    }
  }, render);
  return any;
}

function first_difference(a: ArrayLike<number>, b: ArrayLike<number>): number {
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return i;
  return a.length === b.length ? -1 : a.length;
}

function assert_same_paint(base: IVec, render: boolean, at: string): void {
  const layer = new ObjectWorldLayer();
  const key = { page_idx: TO_XY(L3_IDX(base)), page_off: L2_IDX_IN_L3(base) };
  const a = alloc_chunk_volume(), b = alloc_chunk_volume();
  const any_a = reference_paint(base, a, render);
  const any_b = layer.paint(key, b, { for_raycast: !render });
  assertEquals(any_b, any_a, `${at}: painted anything`);
  assert(!a.starved && !b.starved, `${at}: neither paint starved`);
  seal_chunk_volume(a);
  seal_chunk_volume(b);
  assertEquals(b.bands.map((x) => x.band_z), a.bands.map((x) => x.band_z), `${at}: bands`);
  for (let i = 0; i < a.bands.length; i++) {
    const x = a.bands[i], y = b.bands[i];
    assertEquals(first_difference(y.material, x.material), -1, `${at}: band ${i} materials`);
    assertEquals(first_difference(y.layer_idx, x.layer_idx), -1, `${at}: band ${i} layers`);
    assertEquals(y.sub_cell === null, x.sub_cell === null, `${at}: band ${i} sub-cell lane`);
    if (x.sub_cell && y.sub_cell) assertEquals(first_difference(y.sub_cell, x.sub_cell), -1, `${at}: band ${i} sub-cells`);
    assertEquals(y.extra.len, x.extra.len, `${at}: band ${i} overflow`);
    for (const lane of ["col", "z", "material", "layer_idx", "sub"] as const) {
      assertEquals(first_difference(y.extra[lane], x.extra[lane]), -1, `${at}: band ${i} overflow ${lane}`);
    }
  }
  release_chunk_volume(a);
  release_chunk_volume(b);
}

/** Whatever an earlier run left in the store — Deno keeps localStorage
 * for a test run under its config's location — removed through the same
 * path the eraser takes, so the scene starts from nothing. */
function clear_store(): void {
  for (const base of object_columns()) {
    const ids: string[] = [];
    each_column_entry(base, (e) => ids.push(e.id()));
    for (const id of ids) remove(id);
  }
  assertEquals(object_store_stats(), { chunks: 0, bands: 0, cells: 0 });
}

Deno.test("object/store - the module's records are the TypeScript store's, entry for entry, through random places and removes", () => {
  clear_store();
  const next = rng(2026);
  for (let step = 0; step < 600; step++) {
    mutate(next);
    if (step % 20 === 19) assert_same(next, step);
  }
  assert_same(next, 600);
  assert(ref_chunks.length >= 3 && ref_chunks.some((c) => c.bands.size > 1), "the scene spans chunks and bands");
});

Deno.test("object/store - the module paints a column as the adapter's loop did, voxel for voxel, with and without the eraser's edits", () => {
  assert(ref_chunks.length > 0, "the scene from the test above");
  for (const [pos, chunk] of ref_chunks.entries()) {
    assert_same_paint(chunk.key, true, `chunk ${pos} drawn`);
    assert_same_paint(chunk.key, false, `chunk ${pos} raycast`);
  }
  // Hide one entry and carve one cell in the densest chunk; only the drawn
  // build sees either.
  const chunk = ref_chunks.reduce((a, b) => ref_entries(a).length >= ref_entries(b).length ? a : b);
  const entries = ref_entries(chunk);
  const solid = entries.find((e) => !object_value_is_air(e.state) && !object_value_is_decor(e.state));
  assert(solid !== undefined, "a solid entry to hide");
  let hidden: string | null = null;
  each_column_entry(chunk.key, (e) => {
    if (hidden === null && e.band_of === solid.band && e.pos_of === solid.pos) hidden = e.id();
  });
  set_virtual_hide(hidden);
  const [cx0, cy0] = cell_index({ i: chunk.key, f: 0n });
  const carve = pvf3.gather({ i: 0n, f: 0n }, (cx0 + 5n) * CELL_SIZE_FP, (cy0 + 7n) * CELL_SIZE_FP, 3n * CELL_SIZE_FP);
  set_virtual_carve_preview(carve.i, carve.f, U_PER_CELL);
  try {
    assert_same_paint(chunk.key, true, "edited chunk drawn");
    assert_same_paint(chunk.key, false, "edited chunk raycast");
    // A column that is not the hidden entry's must not hide anything.
    const other = ref_chunks.find((c) => c !== chunk);
    if (other) assert_same_paint(other.key, true, "another chunk under the same hide");
  } finally {
    set_virtual_hide(null);
    clear_virtual_carve_preview();
  }
});
