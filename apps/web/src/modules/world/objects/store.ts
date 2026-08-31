// @pxl/web/modules/world/objects/store.ts

/**
 * Object storage + spatial index. No "fine at current counts" here — this
 * is sized to work the same way at a handful of objects and at millions:
 * every mutation touches exactly the one L1 cell it affects (never a
 * whole-world walk), and every query is scoped to a cell or chunk (never a
 * scan over every placed object anywhere).
 *
 * Two layers:
 * - `cells`: one sparse_tree per occupied L1 cell — unchanged from before,
 *   already O(1)-ish per mutation.
 * - `objects_by_cell`/`entries_by_l2_column`: derived spatial indices kept
 *   OUTSIDE zustand's own reactive state (same reasoning terrain/store.ts's
 *   own "interests" registry uses: this changes on every place()/remove(),
 *   and diffing/copying a big Map on every mutation just to satisfy
 *   zustand's own immutability convention would reintroduce the exact
 *   O(total objects) cost this file exists to avoid). Reactivity is a
 *   small keyed listener registry instead — a component subscribed to one
 *   chunk/cell key is notified only when THAT key's own objects actually
 *   changed, never on an unrelated placement elsewhere in the world.
 */

import { useSyncExternalStore } from "react";

import type { f64 } from "@xpute/core/kit/word.ts";
import type { U8Array } from "@xpute/core/kit/array.ts";
import type { FVec, IVec, PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { b64url_to_bytes, bytes_to_b64url } from "@xpute/core/kit/encoding.ts";

import { OBJECT_LAYOUT, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { object_color_id_from_num, object_color_num_id, object_types, OBJECT_UNDEF, object_value_is_air, object_value_is_present, type ObjectColorId } from "@pxl/shared/spec/cell/objects.ts";
import * as sparse_tree from "@pxl/shared/spec/space/sparse_tree/tree.ts";
import type { SparseTree } from "@pxl/shared/spec/space/sparse_tree/tree.ts";
import { L2_IDX_IN_L3, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";

type ObjectEntryBase = { id: string; i: IVec; f: FVec; size: f64 };
export type PlacedObject = ObjectEntryBase & { kind: "solid"; color_id: ObjectColorId };
export type ObjectAir = ObjectEntryBase & { kind: "air" };
export type ObjectLayerEntry = PlacedObject | ObjectAir;

const STORAGE_KEY = "pxl:objects:v4";
const EMPTY: readonly PlacedObject[] = [];
const EMPTY_ENTRIES: readonly ObjectLayerEntry[] = [];

// ---- Cell tree storage (unchanged shape) ----

type ObjectTree = SparseTree<U8Array>;
let cells = new Map<string, ObjectTree>();

const CELL_SIZE_FP = pvf3.from_real(U_PER_CELL);

/** BigInt has no floor-div — `/` truncates toward zero, which is wrong for negative coordinates (the X axis is signed/cyclic, see indexer.ts). CELL_SIZE_FP is always positive, so only `a`'s sign matters. */
function floor_div(a: bigint, b: bigint): bigint {
  const q = a / b;
  return a % b !== 0n && a < 0n ? q - 1n : q;
}

/** Exported for callers that need to probe exact-cell occupancy directly
 * (get_objects_in_cell) instead of get_objects_near's ±1-cell neighborhood —
 * e.g. decor/decor_layer.tsx, which must tell "no object in this exact
 * ground cell" apart from "an object one cell over" to decide whether a
 * scattered decoration (a tree, say) should render there. */
export function cell_index(coord: PVF3): [bigint, bigint, bigint] {
  return [
    floor_div(pvf3.X(coord), CELL_SIZE_FP),
    floor_div(pvf3.Y(coord), CELL_SIZE_FP),
    floor_div(pvf3.Z(coord), CELL_SIZE_FP),
  ];
}

export function cell_key(ix: bigint, iy: bigint, iz: bigint): string {
  return `${ix},${iy},${iz}`;
}

function parse_cell_key(key: string): [bigint, bigint, bigint] {
  const [ix, iy, iz] = key.split(",");
  return [BigInt(ix), BigInt(iy), BigInt(iz)];
}

/** Absolute PVF3 corner (min-x/y/z) of cell (ix,iy,iz). */
function cell_corner(ix: bigint, iy: bigint, iz: bigint): PVF3 {
  return pvf3.gather({ i: 0n, f: 0n }, ix * CELL_SIZE_FP, iy * CELL_SIZE_FP, iz * CELL_SIZE_FP);
}

/** Which renderer-owned 64-wide L2 XY column contains this cell. */
function l2_column_key_of_cell(ix: bigint, iy: bigint, iz: bigint): string {
  const corner = cell_corner(ix, iy, iz);
  const page_idx = L3_IDX(corner.i);
  const page_off = L2_IDX_IN_L3(corner.i);
  return `${page_idx.toString()}:${page_off.toString()}`;
}

/** Same renderer L2-column key shape l2_column_key_of_cell produces. */
export function l2_column_key_of(page_idx: IVec, page_off: IVec): string {
  return `${page_idx.toString()}:${L2_IDX_IN_L3(page_off).toString()}`;
}

function new_tree(): ObjectTree {
  return sparse_tree.alloc(OBJECT_LAYOUT, 8, Uint8Array);
}

/** Deterministic id for a walked leaf block — encodes exactly what remove() needs to locate it again (including its own cell key, so remove() never needs a reverse lookup), no random/stored identity required. */
function leaf_id(key: string, x: number, y: number, z: number, level: number): string {
  return `${key}:${x},${y},${z}:${level}`;
}

function parse_leaf_id(id: string): { key: string; x: number; y: number; z: number; level: number } | null {
  const m = /^(-?\d+,-?\d+,-?\d+):(-?\d+),(-?\d+),(-?\d+):(-?\d+)$/.exec(id);
  if (!m) return null;
  return { key: m[1], x: Number(m[2]), y: Number(m[3]), z: Number(m[4]), level: Number(m[5]) };
}

/** Real-unit local offset (x,y,z at min-level lattice units, see sparse_tree's walk() doc comment) as a PVF3, computed via exact bit-shift (pvf3.sh), not a float multiply. */
function local_offset(x: number, y: number, z: number): PVF3 {
  const scalar = pvf3.gather({ i: 0n, f: 0n }, pvf3.from_real(x), pvf3.from_real(y), pvf3.from_real(z));
  return pvf3.sh({ i: 0n, f: 0n }, scalar, OBJECT_LAYOUT.min);
}

/** Walks one cell tree into layer entries, preserving explicit-air leaves. */
function derive_cell_entries(key: string, tree: ObjectTree): ObjectLayerEntry[] {
  const [ix, iy, iz] = parse_cell_key(key);
  const corner = cell_corner(ix, iy, iz);
  const out: ObjectLayerEntry[] = [];

  sparse_tree.walk(tree, (x, y, z, level, val) => {
    if (val === OBJECT_UNDEF) return;
    if (!object_value_is_present(val) || val === object_types.block.UNKNOWN) return;
    const abs = pvf3.add({ i: 0n, f: 0n }, corner, local_offset(x, y, z));
    const base: ObjectEntryBase = {
      id: leaf_id(key, x, y, z, level),
      i: abs.i,
      f: abs.f,
      size: 2 ** level,
    };
    out.push(object_value_is_air(val) ? { ...base, kind: "air" } : { ...base, kind: "solid", color_id: object_color_id_from_num(val) });
  });

  return out;
}

// ---- Spatial index (outside zustand — see this file's own doc comment) ----

const objects_by_cell = new Map<string, PlacedObject[]>();
const entries_by_l2_column = new Map<string, ObjectLayerEntry[]>();
/** Monotonic per-render-L2-column revision. */
const l2_column_revisions = new Map<string, number>();

/** Live per-key listener registrations — same "don't wake every consumer on every change" reasoning as terrain/store.ts's own interests registry, just keyed directly instead of predicate-matched (every subscriber here already knows its own exact cell/chunk key). */
const cell_listeners = new Map<string, Set<() => void>>();
const l2_column_listeners = new Map<string, Set<() => void>>();

function subscribe(registry: Map<string, Set<() => void>>, key: string, listener: () => void): () => void {
  let set = registry.get(key);
  if (!set) {
    set = new Set();
    registry.set(key, set);
  }
  set.add(listener);
  return () => {
    set!.delete(listener);
    if (set!.size === 0) registry.delete(key);
  };
}

function notify(registry: Map<string, Set<() => void>>, key: string): void {
  const set = registry.get(key);
  if (!set) return;
  for (const listener of set) listener();
}

/** Re-derives one cell and refreshes only its collision and L2-column buckets. */
function reindex_cell(ix: bigint, iy: bigint, iz: bigint): void {
  const key = cell_key(ix, iy, iz);
  const l2_key = l2_column_key_of_cell(ix, iy, iz);
  const tree = cells.get(key);
  const fresh_entries = tree ? derive_cell_entries(key, tree) : [];
  const fresh_objects = fresh_entries.filter((entry): entry is PlacedObject => entry.kind === "solid");

  if (fresh_objects.length > 0) objects_by_cell.set(key, fresh_objects);
  else objects_by_cell.delete(key);

  const l2_entries = entries_by_l2_column.get(l2_key) ?? [];
  const l2_rest = l2_entries.filter((entry) => !entry.id.startsWith(`${key}:`));
  const next_l2 = fresh_entries.length > 0 ? [...l2_rest, ...fresh_entries] : l2_rest;
  if (next_l2.length > 0) entries_by_l2_column.set(l2_key, next_l2);
  else entries_by_l2_column.delete(l2_key);

  l2_column_revisions.set(l2_key, (l2_column_revisions.get(l2_key) ?? 0) + 1);
  notify(cell_listeners, key);
  notify(l2_column_listeners, l2_key);
}

/** Every object currently claiming cell `key`, or EMPTY — O(1) lookup, non-reactive (read live state directly, same convention WorldLayer.flatten's own doc comment establishes for every other backing store). */
export function get_objects_in_cell(key: string): readonly PlacedObject[] {
  return objects_by_cell.get(key) ?? EMPTY;
}

/** All layer entries in one renderer-owned 64-wide L2 XY column. */
export function get_object_entries_in_l2_column(l2_key: string): readonly ObjectLayerEntry[] {
  return entries_by_l2_column.get(l2_key) ?? EMPTY_ENTRIES;
}

/** Objects near a candidate corner+size — the cell it's in plus the 26 neighbors, since a candidate near a cell face can legitimately need to see an object one cell over (collision.ts's find_overlapping_object, ray.ts's resolve_hit_object). 27 O(1) lookups, still independent of total object count. */
export function get_objects_near(corner: PVF3, _size: f64): readonly PlacedObject[] {
  const [cx, cy, cz] = cell_index(corner);
  // size can push the candidate's own far corner into the NEXT cell too —
  // covered by the same 3x3x3 neighborhood as long as size <= one cell
  // (objects/size.ts's own largest tier), which every placeable size is.
  const out: PlacedObject[] = [];
  for (let dx = -1n; dx <= 1n; dx++) {
    for (let dy = -1n; dy <= 1n; dy++) {
      for (let dz = -1n; dz <= 1n; dz++) {
        const bucket = objects_by_cell.get(cell_key(cx + dx, cy + dy, cz + dz));
        if (bucket) out.push(...bucket);
      }
    }
  }
  return out;
}

/** One object by id, or null — parses the id's own embedded cell key (leaf_id's own doc comment) instead of a reverse index, so this is a single O(objects in one cell) lookup, not O(total objects). */
export function get_placed_object(id: string): PlacedObject | null {
  const parsed = parse_leaf_id(id);
  if (!parsed) return null;
  return objects_by_cell.get(parsed.key)?.find((o) => o.id === id) ?? null;
}

/** One renderer L2 column subscribes to one primitive revision. */
export function useObjectL2ColumnRevision(l2_key: string): number {
  return useSyncExternalStore(
    (on_change) => subscribe(l2_column_listeners, l2_key, on_change),
    () => l2_column_revisions.get(l2_key) ?? 0,
  );
}

/** React hook: same as useObjectsInChunk, scoped to one object's own hovered_object_id lookup — re-renders only when that object's own cell changes (placed/removed/moved), not on unrelated world activity. */
export function usePlacedObject(id: string | null): PlacedObject | null {
  const parsed = id ? parse_leaf_id(id) : null;
  return useSyncExternalStore(
    (on_change) => parsed ? subscribe(cell_listeners, parsed.key, on_change) : () => {},
    () => id ? get_placed_object(id) : null,
  );
}

// ---- Persistence (same structural wire format; v4 full-lane coordinate scale) ----

// Packed binary, one blob per occupied cell (each blob is that cell's own
// sparse_tree.encode() output) rather than one flat per-object record —
// matches the tree-per-cell storage model directly instead of re-flattening
// it. Layout, little-endian throughout:
//   [0..4)  u32  cell_count
//   [4..8)  u32  reserved (keeps the header 8-byte aligned, matching every
//                per-cell entry below)
//   per cell (24 + tree_blob_nbyte bytes), `cell_count` times:
//     [0..8)   i64  ix
//     [8..16)  i64  iy
//     [16..24) i64  iz
//     [24..28) u32  tree_blob_word_len (sparse_tree.encode()'s own U64Array length)
//     [28..32) u32  reserved (realigns to 8 after the u32 length)
//     [32..)   tree_blob_word_len * 8 bytes — sparse_tree.encode(tree)
function encode_objects(): string {
  const blobs: { ix: bigint; iy: bigint; iz: bigint; blob: BigUint64Array }[] = [];
  let total = 8;
  for (const [key, tree] of cells) {
    const [ix, iy, iz] = parse_cell_key(key);
    const blob = sparse_tree.encode(tree);
    blobs.push({ ix, iy, iz, blob });
    total += 24 + 8 + blob.byteLength;
  }

  const buf = new ArrayBuffer(total);
  const view = new DataView(buf);
  const bytes = new Uint8Array(buf);
  view.setUint32(0, blobs.length, true);

  let offset = 8;
  for (const { ix, iy, iz, blob } of blobs) {
    view.setBigInt64(offset, ix, true);
    view.setBigInt64(offset + 8, iy, true);
    view.setBigInt64(offset + 16, iz, true);
    view.setUint32(offset + 24, blob.length, true);
    offset += 32;
    bytes.set(new Uint8Array(blob.buffer, blob.byteOffset, blob.byteLength), offset);
    offset += blob.byteLength;
  }

  return bytes_to_b64url(bytes);
}

function decode_objects(blob: string): Map<string, ObjectTree> {
  const bytes = b64url_to_bytes(blob);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const count = view.getUint32(0, true);

  const out = new Map<string, ObjectTree>();
  let offset = 8;
  for (let n = 0; n < count; n++) {
    const ix = view.getBigInt64(offset, true);
    const iy = view.getBigInt64(offset + 8, true);
    const iz = view.getBigInt64(offset + 16, true);
    const word_len = view.getUint32(offset + 24, true);
    offset += 32;

    const byte_len = word_len * 8;
    const tree_buf = bytes.buffer.slice(bytes.byteOffset + offset, bytes.byteOffset + offset + byte_len);
    const tree = sparse_tree.decode(new BigUint64Array(tree_buf), OBJECT_LAYOUT, Uint8Array);
    out.set(cell_key(ix, iy, iz), tree);
    offset += byte_len;
  }

  return out;
}

function load(): Map<string, ObjectTree> {
  if (typeof localStorage === "undefined") return new Map();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return new Map();
    return decode_objects(raw);
  } catch {
    return new Map();
  }
}

function save(): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(STORAGE_KEY, encode_objects());
}

cells = load();
// Initial index build — O(total objects), paid once at module load, same
// as the old derive_objects() paid on every single mutation before this.
for (const [key] of cells) {
  const [ix, iy, iz] = parse_cell_key(key);
  reindex_cell(ix, iy, iz);
}

// ---- Mutations ----

/**
 * i/f is taken as-is — whatever camera/ray.ts's world_adjacent_corner
 * already computed (the same coordinate the hover preview and overlap
 * check used), never re-derived or re-snapped here. One source of truth
 * for where a placement goes, not a second one that quietly overrides it.
 */
export function place(i: IVec, f: FVec, size: f64, color_id: ObjectColorId): void {
  const coord: PVF3 = { i, f };

  // size is always an exact power of two by construction (see
  // objects/size.ts's OBJECT_SIZES) — log2 is exact here, Math.round just
  // guards against float noise, not real rounding. level===max (the
  // whole-cell tier) is out of write()'s addressable range on purpose —
  // the tree's root can never be a scalar leaf (sparse_tree's own
  // invariant #4), so "the whole cell is uniformly this value" is exactly
  // what fill() exists for instead.
  const level = Math.round(Math.log2(size));
  const is_whole_cell = level >= OBJECT_LAYOUT.max;

  const [ix, iy, iz] = cell_index(coord);
  const key = cell_key(ix, iy, iz);

  let tree = cells.get(key);
  if (!tree) {
    tree = new_tree();
    cells.set(key, tree);
  }

  const val = object_color_num_id(color_id);
  if (is_whole_cell) {
    sparse_tree.fill(tree, val);
  } else {
    sparse_tree.write(tree, coord, val, level);
    sparse_tree.compact(tree);
  }

  save();
  reindex_cell(ix, iy, iz);
}

export function remove(id: string): void {
  const parsed = parse_leaf_id(id);
  if (!parsed) return;
  const { key } = parsed;

  const tree = cells.get(key);
  if (!tree) return;

  // level===max (the whole-cell tier) — see place()'s matching comment.
  if (parsed.level >= OBJECT_LAYOUT.max) {
    sparse_tree.fill(tree, OBJECT_UNDEF);
  } else {
    const [ix, iy, iz] = parse_cell_key(key);
    const corner = cell_corner(ix, iy, iz);
    const abs = pvf3.add({ i: 0n, f: 0n }, corner, local_offset(parsed.x, parsed.y, parsed.z));

    sparse_tree.write(tree, abs, OBJECT_UNDEF, parsed.level);
    sparse_tree.compact(tree);
  }

  let empty = true;
  sparse_tree.walk(tree, (_x, _y, _z, _level, val) => {
    if (val !== OBJECT_UNDEF) empty = false;
  });
  if (empty) cells.delete(key);

  save();
  const [ix, iy, iz] = parse_cell_key(key);
  reindex_cell(ix, iy, iz);
}
