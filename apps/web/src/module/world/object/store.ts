// @pxl/web/module/world/object/store.ts

/**
 * Object storage + spatial index. No "fine at current counts" here — this
 * is sized to work the same way at a handful of objects and at millions:
 * every mutation touches exactly the one L1 cell it affects (never a
 * whole-world walk), and every query is scoped to a cell or chunk (never a
 * scan over every placed object anywhere).
 *
 * The records are the module's (crates/pxl-core/src/object.rs): one per
 * L2 chunk column holding user objects — a palette, and one sparse octree
 * per 64³ band whose leaves are palette indices — and, derived from them a
 * chunk at a time, the chunk's entries as three lanes and an index of the
 * cells they fall in. A coordinate is lowered here to a chunk key, a band
 * and a lattice position before it crosses, and what comes back is lanes
 * read as views. What stays here is what is string- or platform-shaped:
 * the leaf ids, the eraser's two virtual edits, the change log's bumps and
 * the React subscriptions on them, and the persisted form in localStorage.
 *
 * The index, entry lanes and per-column revisions are derived, kept
 * OUTSIDE zustand's own reactive state (same reasoning terrain/store.ts's
 * own "interests" registry uses: this changes on every place()/remove(),
 * and diffing/copying a big Map on every mutation just to satisfy
 * zustand's own immutability convention would reintroduce the exact
 * O(total objects) cost this file exists to avoid). Reactivity is a
 * small keyed listener registry instead — a component subscribed to one
 * chunk/cell key is notified only when THAT key's own objects actually
 * changed, never on an unrelated placement elsewhere in the world.
 */

import { useSyncExternalStore } from "react";

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";
import type { FVec, IVec, PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { fvec, ivec, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { b64url_to_bytes, bytes_to_b64url } from "@xpute/core/kit/encoding.ts";
import { fnv1a64_str } from "@xpute/core/kit/hash.ts";
import { encoder, TreeReader, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";
import { encode_placements, type Placement, PlacementTable } from "@pxl/shared/record/mod.ts";
import { local_world_to_canonical } from "../frame/coord.ts";

import { OBJECT_LAYOUT, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { shown_ground } from "@pxl/shared/bake/ground.ts";
import {
  cube_types,
  decor_id_from_num,
  decor_num_id,
  type DecorId,
  is_terrain_style_cube,
  object_value_is_air,
  object_value_is_decor,
  placeable_cube_id_from_num,
  placeable_cube_num_id,
  type PlaceableCubeId,
  terrain_style_cube_source,
} from "@pxl/shared/spec/cell/mod.ts";
import { FROM_L3_IDX, L2_BASE, L2_SHAMT, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { CORE_PXL } from "@/render/layer/pipeline/core_pool.ts";
import { CORE_BUILD } from "@/render/layer/pipeline/core_build.ts";
import { bump_world, request_world_tick, world_changes, world_wakers, WorldTopic } from "@/module/world/change.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import { get_ground_type } from "../terrain/elevation.ts";

type ObjectEntryBase = { id: string; i: IVec; f: FVec; size: f64 };
export type PlacedObject = ObjectEntryBase & { kind: "solid"; color_id: PlaceableCubeId };
export type PlacedDecor = ObjectEntryBase & { kind: "decor"; decor_id: DecorId };
export type ObjectAir = ObjectEntryBase & { kind: "air" };
export type ObjectLayerEntry = PlacedObject | PlacedDecor | ObjectAir;

/** Anything a placement/erase/selection query should actually treat as "a
 * real thing occupying this slot" — a solid color cube or a decor object
 * alike, just never `kind: "air"` (that's a virtual erase-marker, not a
 * placed thing). collision.ts/geometry.ts/camera/ray.ts only ever touch the
 * shared ObjectEntryBase fields (i/f/size/id) on this union, never
 * color_id/decor_id, so widening every "which objects are here" query from
 * PlacedObject to this doesn't change any of their own logic. */
export type OccupiedEntry = PlacedObject | PlacedDecor;

// Renamed from "pxl:objects:v4" (2026-09-03) — singular ("object", matching
// this module's own type names PlacedObject/ObjectLayerEntry) since there
// was never a real reason for the old key to be plural — but the number
// still carries forward as v5, not reset to v1: the version count has real
// history (four prior encoding breaks), and the rename is incidental to
// that history, not a reason to erase it. This bump specifically:
// pvf3/core.ts's FRAC_W dropped from 10 to 8 bits, so an existing v4 entry's
// raw `f: FVec` bit-packing no longer means what its own X/Y/Z lane shifts
// assumed; decoding old data under the new layout would silently scramble
// every placed object's fractional position rather than fail loudly. Bump
// this again whenever a future change makes an older entry's encoding mean
// something different from what it meant when it was written. Exported so
// app/setting/section/about_section.tsx can display it next to the tile
// version — both are "which on-disk/on-device format am I reading" facts,
// useful for the same reason (diagnosing a report against a specific
// format generation).
//
// v6 (2026-09-11): a palette per L2 chunk and one tree per 64³ band, whose
// leaves are palette indices, replacing one tree per cell holding catalog
// values directly — the chunk storage section below. Not migrated.
//
// v7 (2026-09-13): one Placement record per page in a page-keyed container
// (the persistence section below) — the same form every placed thing has.
// Not migrated.
export const OBJECT_STORAGE_VERSION: number = 7;
const STORAGE_KEY = `pxl:object:v${OBJECT_STORAGE_VERSION}`;

// ---- Chunk storage ----
//
// One record per L2 chunk column that holds user objects, and inside it one
// sparse octree per 64³ band — the unified cell format of
// issue/20260911-decor-identity-orientation-and-one-cell-format.md.
// The palette is per chunk; a leaf holds a palette index, not a catalog
// value. A state is today's catalog value (cube colour, AIR, decor kind);
// when the decor manifest lands it becomes a resource id's state, and only
// palettes change.
//
// A band tree replaces the tree per cell. It is the unit the mesher already
// reads in (volume.ts's bands are the same 64³), it drops the string-keyed
// map of cells, and whole-cell writes stop needing a special case: the cell
// tier sits inside the tree's range instead of being its root.
//
// The records are the module's; what this section does is lower a
// coordinate to what the module keys by — the chunk (L2_BASE of the XY
// column, passed as the IVec it is), the band's Z, and the position within
// the band in finest-tier steps, which is the low bits of each lane of the
// coordinate shifted to the finest level — and read its lanes back.

/** Cells along one side of a chunk, and of a band. */
const CHUNK_CELLS: bigint = 1n << BigInt(L2_SHAMT);
/** Finest-tier steps along one side of a cell, and of a band. */
const CELL_STEPS: number = 1 << -OBJECT_LAYOUT.min;
const BAND_STEP_MASK: bigint = BigInt(CELL_STEPS) * CHUNK_CELLS - 1n;
const VOID = cube_types.common.VOID;

const CELL_SIZE_FP = pvf3.from_real(U_PER_CELL);

function pxl() {
  if (CORE_PXL === null) throw new Error("object/store: the store needs the core memory and the module over it");
  return CORE_PXL;
}

/** BigInt has no floor-div — `/` truncates toward zero, which is wrong for negative coordinates (the X axis is signed/cyclic, see indexer.ts). CELL_SIZE_FP is always positive, so only `a`'s sign matters. */
function floor_div(a: bigint, b: bigint): bigint {
  const q = a / b;
  return a % b !== 0n && a < 0n ? q - 1n : q;
}

/** Exported for callers that need to probe exact-cell occupancy directly
 * (get_objects_in_cell) instead of get_objects_near's ±1-cell neighborhood —
 * e.g. decor/decor_layer.ts, which must tell "no object in this exact
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

/**
 * The column a cell belongs to, as the key everything about objects is filed
 * under: L2_BASE of the cell's XY column.
 *
 * A column, so Z is dropped before the base is taken. The renderer asks for
 * a column by an XY page index and a page offset built with z = 0, so a key
 * that kept the cell's own Z put every object above absolute z = 64u (about
 * 120 m, one L2 span) under a key nothing ever asked for: stored, indexed,
 * hit-tested and never drawn. Where the terrain already stands at 50 m that
 * was a ceiling of a few dozen blocks.
 */
function column_of_cell(ix: bigint, iy: bigint, iz: bigint): IVec {
  return L2_BASE(TO_XY(cell_corner(ix, iy, iz).i));
}

/** The same key from the renderer's side, a page index and a page offset.
 * Both are normalised to XY, so a caller cannot reopen the gap above by
 * handing in an offset that still carries Z. */
export function object_column_of(page_idx: IVec, page_off: IVec): IVec {
  return L2_BASE(TO_XY(ivec.add(FROM_L3_IDX(TO_XY(page_idx)), TO_XY(page_off))));
}

function band_z_of(iz: bigint): number {
  return Number(floor_div(iz, CHUNK_CELLS));
}

const PVF3_REG: PVF3 = { i: 0n, f: 0n };

/** A coordinate as the module keys it: its column, band, and position
 * within the band in finest-tier steps. One object, reused. */
const $at = { key: 0n as IVec, band: 0, x: 0, y: 0, z: 0 };

function lower(coord: PVF3): typeof $at {
  $at.key = L2_BASE(TO_XY(coord.i));
  $at.band = band_z_of(cell_index(coord)[2]);
  const lattice = pvf3.sh(PVF3_REG, coord, -OBJECT_LAYOUT.min).i;
  $at.x = Number(ivec.X(lattice) & BAND_STEP_MASK);
  $at.y = Number(ivec.Y(lattice) & BAND_STEP_MASK);
  $at.z = Number(ivec.Z(lattice) & BAND_STEP_MASK);
  return $at;
}

/** The state stored at `coord`, VOID where nothing is. */
function read_state(coord: PVF3): number {
  const at = lower(coord);
  return pxl().pxl_object_read(at.key, at.band, at.x, at.y, at.z);
}

/** A chunk's entry lanes, as views over the module's — valid until the
 * next write or reindex; null where the chunk holds nothing. */
function entry_lanes(base: IVec): [Int32Array, Uint32Array, Uint32Array] | null {
  const p = pxl();
  const row_at = p.pxl_object_chunk(base);
  if (row_at === 0) return null;
  const buf = p.memory.buffer;
  const row = new Uint32Array(buf, row_at, 4);
  const n = row[0];
  return [new Int32Array(buf, row[1], n), new Uint32Array(buf, row[2], n), new Uint32Array(buf, row[3], n)];
}

function entry_count(base: IVec): number {
  const p = pxl();
  const row_at = p.pxl_object_chunk(base);
  return row_at === 0 ? 0 : new Uint32Array(p.memory.buffer, row_at, 1)[0];
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

// ---- Entries ----
//
// What a chunk's trees hold, read back as one entry per leaf — a leaf larger
// than a cell split into the cells it covers, since everything downstream
// (collision's 3×3×3 neighbourhood, a size never past one cell) assumes an
// entry is at most a cell, and a band tree merges eight neighbouring cubes
// of one colour into one leaf when it compacts. The module keeps them per
// chunk as three lanes — the band, a packed position, the state — read here
// as views. A tree walked depth first visits a cell's leaves together, so a
// cell's entries are one contiguous run.
//
// `pos` packs the leaf's band-local position in finest-tier steps — 0..511 on
// each axis, nine bits each — and its level above the finest in four, as
// object.rs's pack_pos does.

const POS_BITS = 9;
const POS_MASK = (1 << POS_BITS) - 1;
/** One finest-tier step, and one band's height, in world units. */
const STEP_UNITS: f64 = 2 ** OBJECT_LAYOUT.min;
const BAND_UNITS: f64 = Number(CHUNK_CELLS) * 2 ** OBJECT_LAYOUT.max;

function pack_pos(x: number, y: number, z: number, level: number): number {
  return (x | (y << POS_BITS) | (z << (2 * POS_BITS)) | ((level - OBJECT_LAYOUT.min) << (3 * POS_BITS))) >>> 0;
}

/**
 * One entry as a visit sees it — a single object reused for a whole walk, so
 * reading a column allocates nothing per entry. The chunk-local corner, size
 * and state are plain numbers; `corner()`, `cell()` and `id()` are worked out
 * when asked for, because few callers need them.
 */
export class EntryCursor {
  /** Corner relative to the column's base (z = 0), in world units. */
  lx: f64 = 0;
  ly: f64 = 0;
  lz: f64 = 0;
  size: f64 = 0;
  /** The catalog value: a cube colour, AIR, or a decor kind. */
  state: number = 0;

  private cx0 = 0n;
  private cy0 = 0n;
  private band = 0;
  private pos = 0;
  private x = 0;
  private y = 0;
  private z = 0;
  private level = 0;
  /** The one entry that is not stored — the eraser's carve preview. */
  private fixed: { i: IVec; f: FVec; id: string } | null = null;

  /** Points the cursor at a chunk's first cell; `load` then reads entries. */
  at_chunk(cx0: bigint, cy0: bigint): void {
    this.cx0 = cx0;
    this.cy0 = cy0;
  }

  load(band: number, pos: number, state: number): void {
    this.fixed = null;
    this.band = band;
    this.pos = pos;
    this.x = pos & POS_MASK;
    this.y = (pos >>> POS_BITS) & POS_MASK;
    this.z = (pos >>> (2 * POS_BITS)) & POS_MASK;
    this.level = (pos >>> (3 * POS_BITS)) + OBJECT_LAYOUT.min;
    this.lx = this.x * STEP_UNITS;
    this.ly = this.y * STEP_UNITS;
    this.lz = band * BAND_UNITS + this.z * STEP_UNITS;
    this.size = 2 ** this.level;
    this.state = state;
  }

  /** An entry given by its absolute corner rather than read from lanes. */
  load_fixed(base: IVec, i: IVec, f: FVec, size: f64, state: number, id: string): void {
    this.fixed = { i, f, id };
    const local = ivec.sub(i, base);
    this.lx = ivec.to_real(ivec.X(local)) + fvec.to_real(fvec.X(f));
    this.ly = ivec.to_real(ivec.Y(local)) + fvec.to_real(fvec.Y(f));
    this.lz = ivec.to_real(ivec.Z(local)) + fvec.to_real(fvec.Z(f));
    this.size = size;
    this.state = state;
  }

  /** The cell the entry is in. */
  cell(): [bigint, bigint, bigint] {
    return [
      this.cx0 + BigInt(Math.floor(this.x / CELL_STEPS)),
      this.cy0 + BigInt(Math.floor(this.y / CELL_STEPS)),
      BigInt(this.band) * CHUNK_CELLS + BigInt(Math.floor(this.z / CELL_STEPS)),
    ];
  }

  /** The absolute corner. */
  corner(): PVF3 {
    if (this.fixed) return { i: this.fixed.i, f: this.fixed.f };
    return pvf3.add({ i: 0n, f: 0n }, cell_corner(this.cx0, this.cy0, BigInt(this.band) * CHUNK_CELLS), local_offset(this.x, this.y, this.z));
  }

  id(): string {
    if (this.fixed) return this.fixed.id;
    return leaf_id(cell_key(...this.cell()), this.x, this.y, this.z, this.level);
  }

  /** The entry as the store's own two numbers, for a consumer that keeps
   * instances as lanes (decor/instance.ts) and rebuilds the id on demand
   * with entry_id_of. A fixed entry has no lane form and reports NO_ENTRY. */
  get band_of(): number {
    return this.fixed ? 0xffffffff : this.band;
  }
  get pos_of(): number {
    return this.fixed ? 0 : this.pos;
  }

  /** The entry as an object, for the few callers that keep one. */
  object(): ObjectLayerEntry {
    const { i, f } = this.corner();
    const base: ObjectEntryBase = { id: this.id(), i, f, size: this.size };
    if (object_value_is_decor(this.state)) return { ...base, kind: "decor", decor_id: decor_id_from_num(this.state) };
    return object_value_is_air(this.state) ? { ...base, kind: "air" } : { ...base, kind: "solid", color_id: placeable_cube_id_from_num(this.state) };
  }
}

/** Column walks and cell lookups each have their own, so a cell lookup made
 * while nothing else is walking cannot clobber a walk in progress. */
const $cursor = new EntryCursor();
const $cell_cursor = new EntryCursor();
const $edit_cursor = new EntryCursor();

/** The cell key a leaf id starts with — see leaf_id. */
function entry_cell_key(id: string): string {
  return id.slice(0, id.indexOf(":"));
}

// ---- Index (outside zustand — see this file's own doc comment) ----
//
// Cells by their corner, each naming the run of its chunk's entries it
// covers — the module's, rebuilt a chunk at a time by reindex_chunk, which
// then bumps the revisions of the column and of every cell that changed.

/** The run [start, start + count) of the column's entries in the cell at
 * band-local (cx, cy) and absolute cell Z, and how many of them really
 * occupy it — every kind but AIR, which is an erase-marker rather than a
 * thing (OccupiedEntry). One object, reused; null where the cell holds
 * nothing. */
const $cell = { start: 0, count: 0, solid: 0 };

function cell_run(ix: bigint, iy: bigint, iz: bigint): typeof $cell | null {
  const base = column_of_cell(ix, iy, iz);
  const [cx0, cy0] = cell_index({ i: base, f: 0n });
  const p = pxl();
  const row_at = p.pxl_object_cell(base, Number(ix - cx0), Number(iy - cy0), Number(iz));
  if (row_at === 0) return null;
  const row = new Uint32Array(p.memory.buffer, row_at, 3);
  $cell.start = row[0];
  $cell.count = row[1];
  $cell.solid = row[2];
  return $cell;
}

/**
 * Per-column revisions, two per column — cube content and decor — kept apart
 * so a decor-only change (hovering the eraser over a placed tree) never
 * forces the chunk to rescan and rebake its cubes — visible as a flicker,
 * 2026-09-05 — and a solid-only change never rebuilds decor for nothing.
 * They are the world change log's (module/world/change.ts), whose revisions
 * come from one tick per topic and never repeat for a key, so a column that
 * empties and fills again cannot hand a chunk a revision it already saw.
 */

/** Moves on every write to any column, so a reader that cached a derived
 * answer can tell whether the store has changed under it at all. */
export function objects_revision(): number {
  return world_changes.tick(WorldTopic.OBJECT_COLUMN) + world_changes.tick(WorldTopic.DECOR_COLUMN);
}

function bump_column(base: IVec, objects: boolean, decor: boolean): void {
  if (objects) bump_world(WorldTopic.OBJECT_COLUMN, base);
  if (decor) bump_world(WorldTopic.DECOR_COLUMN, base);
}

/** Hovered-object snapshots, by id — useSyncExternalStore needs the same
 * object back while nothing changed, and an entry is made fresh each time it
 * is read. Dropped when their cell changes. */
const placed_snapshots = new Map<string, OccupiedEntry | null>();

/** Re-derives one chunk's entries and cells in the module, which records
 * the change itself — the column in both of its topics and every cell whose
 * entries changed — then drops the snapshots of those cells and asks for
 * the frame that delivers it. */
function reindex_chunk(base: IVec): void {
  const log = world_changes;
  const from = log.head;
  pxl().pxl_object_reindex(base);
  for (let c = from; c < log.head; c++) {
    const i = log.slot(c);
    if (log.topic[i] !== WorldTopic.OBJECT_CELL) continue;
    const corner = BigInt.asUintN(64, log.key[i]);
    for (const id of placed_snapshots.keys()) {
      if (cell_corner(...parse_cell_key(entry_cell_key(id))).i === corner) placed_snapshots.delete(id);
    }
  }
  request_world_tick();
}

/**
 * Every entry of a column, through one reused cursor — see EntryCursor.
 * `render` applies the eraser preview's two edits, the hidden entry left out
 * and the carve preview put in, which only the drawn build may see (see
 * set_virtual_hide). A visit must not walk this store again: the cursor is
 * shared.
 */
/**
 * The string id of the entry `(band, pos)` under column `base` — what a
 * consumer that kept an instance as lanes (decor/instance.ts) asks for
 * when a hit on that instance has to resolve to a selectable entry. The
 * same numbers the store's own lanes hold, through the same cursor packing.
 */
export function entry_id_of(base: IVec, band: number, pos: number): string {
  const [cx0, cy0] = cell_index({ i: base, f: 0n });
  $cell_cursor.at_chunk(cx0, cy0);
  $cell_cursor.load(band, pos, 0);
  return $cell_cursor.id();
}

/** Whether any entry — including the carve preview — is filed under a
 * column. One map lookup; what a coarse build asks before skipping the scan. */
export function column_has_entries(base: IVec): boolean {
  if (entry_count(base) > 0) return true;
  return virtual_carve_preview !== null && virtual_carve_preview.column === base;
}

export function each_column_entry(base: IVec, visit: (entry: EntryCursor) => void, render: boolean = false): void {
  const lanes = entry_lanes(base);
  if (lanes !== null) {
    const [bands, poss, states] = lanes;
    const [cx0, cy0] = cell_index({ i: base, f: 0n });
    $cursor.at_chunk(cx0, cy0);
    for (let k = 0; k < bands.length; k++) {
      $cursor.load(bands[k], poss[k], states[k]);
      if (render && virtual_hide_id !== null && $cursor.id() === virtual_hide_id) continue;
      visit($cursor);
    }
  }
  const carve = virtual_carve_preview;
  if (render && carve !== null && carve.column === base) {
    $cursor.load_fixed(base, carve.i, carve.f, carve.size, cube_types.common.AIR, carve.id);
    visit($cursor);
  }
}

/**
 * The drawn build's two edits to a column, as the module's paint takes
 * them: the hidden entry by its band and packed position — only when it is
 * this column's, since the same numbers name a different entry in every
 * other column — and the carve preview by its corner relative to the
 * column's base. One object, reused.
 */
export type ColumnEdits = { hide: 0 | 1; hide_band: i32; hide_pos: u32; carve: 0 | 1; lx: f64; ly: f64; lz: f64; size: f64 };
const $edits: ColumnEdits = { hide: 0, hide_band: 0, hide_pos: 0, carve: 0, lx: 0, ly: 0, lz: 0, size: 0 };

export function column_render_edits(base: IVec): ColumnEdits {
  $edits.hide = 0;
  $edits.carve = 0;
  const hide = virtual_hide_id === null ? null : parse_leaf_id(virtual_hide_id);
  if (hide !== null) {
    const [ix, iy, iz] = parse_cell_key(hide.key);
    if (column_of_cell(ix, iy, iz) === base) {
      $edits.hide = 1;
      $edits.hide_band = band_z_of(iz);
      $edits.hide_pos = pack_pos(hide.x, hide.y, hide.z, hide.level);
    }
  }
  const carve = virtual_carve_preview;
  if (carve !== null && carve.column === base) {
    $cursor.load_fixed(base, carve.i, carve.f, carve.size, cube_types.common.AIR, carve.id);
    $edits.carve = 1;
    $edits.lx = $cursor.lx;
    $edits.ly = $cursor.ly;
    $edits.lz = $cursor.lz;
    $edits.size = carve.size;
  }
  return $edits;
}

function each_cell_entry(ix: bigint, iy: bigint, iz: bigint, visit: (entry: EntryCursor) => void): void {
  const run = cell_run(ix, iy, iz);
  if (run === null) return;
  const base = column_of_cell(ix, iy, iz);
  const [bands, poss, states] = entry_lanes(base)!;
  const [cx0, cy0] = cell_index({ i: base, f: 0n });
  $cell_cursor.at_chunk(cx0, cy0);
  const end = run.start + run.count;
  for (let k = run.start; k < end; k++) {
    $cell_cursor.load(bands[k], poss[k], states[k]);
    visit($cell_cursor);
  }
}

function occupants(ix: bigint, iy: bigint, iz: bigint, out: OccupiedEntry[]): void {
  each_cell_entry(ix, iy, iz, (entry) => {
    if (!object_value_is_air(entry.state)) out.push(entry.object() as OccupiedEntry);
  });
}

/** Every object claiming cell `key` — read live, non-reactive, same
 * convention WorldLayer.flatten's own doc comment sets for backing stores. */
export function get_objects_in_cell(key: string): readonly OccupiedEntry[] {
  const out: OccupiedEntry[] = [];
  occupants(...parse_cell_key(key), out);
  return out;
}

/** Whether anything really occupies the cell — what scattered decor asks
 * before standing in it, without making an object to find out. */
export function cell_is_occupied(ix: bigint, iy: bigint, iz: bigint): boolean {
  const run = cell_run(ix, iy, iz);
  return run !== null && run.solid > 0;
}

/** Objects near a candidate corner+size — the cell it's in plus the 26 neighbors, since a candidate near a cell face can legitimately need to see an object one cell over (collision.ts's find_overlapping_object, ray.ts's resolve_hit_object). 27 O(1) lookups, still independent of total object count. */
export function get_objects_near(corner: PVF3, _size: f64): readonly OccupiedEntry[] {
  const [cx, cy, cz] = cell_index(corner);
  // size can push the candidate's own far corner into the NEXT cell too —
  // covered by the same 3x3x3 neighborhood as long as size <= one cell
  // (object/size.ts's own largest tier), which every placeable size is.
  const out: OccupiedEntry[] = [];
  for (let dx = -1n; dx <= 1n; dx++) {
    for (let dy = -1n; dy <= 1n; dy++) {
      for (let dz = -1n; dz <= 1n; dz++) occupants(cx + dx, cy + dy, cz + dz, out);
    }
  }
  return out;
}

/** One object by id, or null — the id carries its cell (leaf_id), so this
 * is one cell's entries, not every object's. */
export function get_placed_object(id: string): OccupiedEntry | null {
  const parsed = parse_leaf_id(id);
  if (!parsed) return null;
  let found = null as OccupiedEntry | null;
  each_cell_entry(...parse_cell_key(parsed.key), (entry) => {
    if (found === null && !object_value_is_air(entry.state) && entry.id() === id) found = entry.object() as OccupiedEntry;
  });
  return found;
}

/** The one object painted as absent from the real render build — see
 * set_virtual_hide. Module state, not zustand: read from the synchronous
 * chunk build, not a React render. */
let virtual_hide_id: string | null = null;

/** Which entry set_virtual_hide is currently suppressing, for the one
 * consumer that has to keep drawing it in some form — decor, whose hit
 * testing runs against the very mesh the hide would remove. */
export function virtual_hidden_id(): string | null {
  return virtual_hide_id;
}

/**
 * Makes exactly one placed object paint as if it didn't exist, for the real
 * GPU-mesh build ONLY — the eraser's hover preview, which needs the real
 * target to stop rendering (so what is behind it shows) with a translucent
 * ghost of it on top.
 *
 * Deliberately does NOT affect the cell queries (collision, decor's
 * occupancy) or a raycast paint: the object has not been removed, and the
 * raycaster is what resolves the hover in the first place — if it stopped
 * seeing the hidden object, hover would clear, the hide would lift, the
 * raycaster would hit it again: an infinite loop ("Maximum update depth
 * exceeded", 2026-09-03). Bumps the column's revision, the same path a real
 * place()/remove() takes. The brief overlap and gap while the rebuild lands
 * is accepted — see issue/20260903-terrain-decor-render-unification.md.
 */
export function set_virtual_hide(id: string | null): void {
  const previous = virtual_hide_id;
  if (previous === id) return;
  virtual_hide_id = id;
  send_render_edits();
  if (previous) bump_for_object(previous);
  if (id) bump_for_object(id);
}

/** Bumps only the revision relevant to `id`'s kind — decor or cube content
 * — so hover churn never rebakes the wrong half; both when the kind is gone
 * already (erased while hidden), which costs one extra bump on each side. */
function bump_for_object(id: string): void {
  const parsed = parse_leaf_id(id);
  if (!parsed) return;
  const [ix, iy, iz] = parse_cell_key(parsed.key);
  const kind = get_placed_object(id)?.kind ?? null;
  bump_column(column_of_cell(ix, iy, iz), kind !== "decor", kind !== "solid");
}

/**
 * The eraser's "dig into bare terrain" preview — the terrain-side twin of
 * virtual_hide_id. There is no leaf to hide when the hover target is real
 * terrain, so this injects one AIR entry into the render build that was
 * never written — same render-only scoping as virtual_hide_id, for the same
 * reason. `i`/`f`/`size` match what a real place_air() at this hover would
 * write, so the preview renders identically to what it stands in for.
 */
let virtual_carve_preview: { key: string; column: IVec; i: IVec; f: FVec; size: f64; id: string } | null = null;

export function set_virtual_carve_preview(i: IVec, f: FVec, size: f64): void {
  const [ix, iy, iz] = cell_index({ i, f });
  const key = cell_key(ix, iy, iz);
  if (virtual_carve_preview?.key === key) return;
  const previous = virtual_carve_preview;
  virtual_carve_preview = { key, column: column_of_cell(ix, iy, iz), i, f, size, id: `virtual-carve:${key}` };
  send_render_edits();
  if (previous) bump_column(previous.column, true, false);
  bump_column(virtual_carve_preview.column, true, false);
}

export function clear_virtual_carve_preview(): void {
  if (!virtual_carve_preview) return;
  const { column } = virtual_carve_preview;
  virtual_carve_preview = null;
  send_render_edits();
  bump_column(column, true, false);
}

/** Tells the module's builds the two edits, before the bumps that rebuild
 * the columns they touch reach a chunk. */
function send_render_edits(): void {
  const hide = virtual_hide_id === null ? null : parse_leaf_id(virtual_hide_id);
  let hide_edit = null;
  if (hide !== null) {
    const [ix, iy, iz] = parse_cell_key(hide.key);
    hide_edit = { column: column_of_cell(ix, iy, iz), band: band_z_of(iz), pos: pack_pos(hide.x, hide.y, hide.z, hide.level) };
  }
  const carve = virtual_carve_preview;
  let carve_edit = null;
  if (carve !== null) {
    $edit_cursor.load_fixed(carve.column, carve.i, carve.f, carve.size, cube_types.common.AIR, carve.id);
    carve_edit = { column: carve.column, lx: $edit_cursor.lx, ly: $edit_cursor.ly, lz: $edit_cursor.lz, size: carve.size };
  }
  CORE_BUILD.edits(hide_edit, carve_edit);
}

/** Cube-content revision of a column — a decor-only change never bumps it. */
export function object_column_revision(base: IVec): number {
  return world_changes.revision_of(WorldTopic.OBJECT_COLUMN, base);
}

/** Decor revision of a column — a solid-only change never bumps it. */
export function decor_column_revision(base: IVec): number {
  return world_changes.revision_of(WorldTopic.DECOR_COLUMN, base);
}

export function subscribe_object_column(base: IVec, on_change: () => void): () => void {
  return world_wakers.subscribe(WorldTopic.OBJECT_COLUMN, base, on_change);
}

export function subscribe_decor_column(base: IVec, on_change: () => void): () => void {
  return world_wakers.subscribe(WorldTopic.DECOR_COLUMN, base, on_change);
}

/** React hook: one hovered object, re-rendering only when its own cell
 * changes, not on unrelated world activity. */
export function usePlacedObject(id: string | null): OccupiedEntry | null {
  const parsed = id ? parse_leaf_id(id) : null;
  return useSyncExternalStore(
    (on_change) => parsed ? world_wakers.subscribe(WorldTopic.OBJECT_CELL, cell_corner(...parse_cell_key(parsed.key)).i, on_change) : () => {},
    () => {
      if (!id) return null;
      if (!placed_snapshots.has(id)) placed_snapshots.set(id, get_placed_object(id));
      return placed_snapshots.get(id) ?? null;
    },
  );
}

// ---- Persistence ----

// One Placement record per page, in a container that names the page:
//   [page_idx u64, placements pkt, page_idx u64, placements pkt, ...]
// The record is @pxl/shared/record/placement.ts's — the one form every
// placed thing has — with positions page-local to FROM_L3_IDX(page_idx).
// A row's value is the catalog value the store writes (a block colour,
// AIR, a decor kind), which is its palette id; its id is the hash of the
// entry's own id. The store carries no heading: a placed decor's facing
// is derived from its position (decor_layer.ts's user_decor_pick), so the
// row's heading is written as zero and not read back.
//
// Positions round-trip exactly: the store's steps are binary fractions of
// a cell, and a page-local f32 holds any such value under 1024
// (issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).

const $local: V3 = [0, 0, 0];
const $abs: PVF3 = { i: 0n, f: 0n };

/** Set while a blob is being replayed into the store, so the writes it
 * makes neither save (the blob is what is being loaded) nor reindex per
 * row (the load reindexes every chunk once, after). */
let restoring = false;

/** The columns holding objects, in the module's order — for a walk over
 * everything placed, which is what a save is. */
export function object_columns(): IVec[] {
  const p = pxl();
  const keys: IVec[] = [];
  for (let i = 0, n = p.pxl_object_chunks(); i < n; i++) keys.push(BigInt.asUintN(64, p.pxl_object_chunk_key(i)));
  return keys;
}

export function encode_objects(): string {
  const by_page = new Map<IVec, Placement[]>();
  for (const base of object_columns()) {
    const page_idx = TO_XY(L3_IDX(base));
    const page_base = FROM_L3_IDX(page_idx);
    let rows = by_page.get(page_idx);
    if (!rows) by_page.set(page_idx, rows = []);
    each_column_entry(base, (entry) => {
      const corner = entry.corner();
      const local: PVF3 = { i: ivec.sub(corner.i, page_base), f: corner.f };
      rows.push({
        id: fnv1a64_str(entry.id()),
        x: pvf3.to_real(pvf3.X(local)),
        y: pvf3.to_real(pvf3.Y(local)),
        z: pvf3.to_real(pvf3.Z(local)),
        size: entry.size,
        heading: 0,
        value: entry.state,
      });
    });
  }
  const pkt = encoder.encode(
    new TreeView().branch((__) => {
      for (const [page_idx, rows] of by_page) {
        if (rows.length === 0) continue;
        __.u64(page_idx).graft(encode_placements(rows));
      }
    }),
    { init_cap: 1 << 14 },
  );
  return bytes_to_b64url(pkt);
}

export function decode_objects(blob: string): void {
  const bytes = b64url_to_bytes(blob);
  const root = new TreeReader(bytes).read_branch();
  restoring = true;
  try {
    for (let k = 0; k + 1 < root.len; k += 2) {
      const page_idx = BigInt.asUintN(64, root.at(k).get<bigint>());
      const page_base = FROM_L3_IDX(page_idx);
      const table = new PlacementTable(root.at(k + 1).as_branch());
      for (let r = 0; r < table.count; r++) {
        $local[0] = table.x[r];
        $local[1] = table.y[r];
        $local[2] = table.z[r];
        const corner = local_world_to_canonical($abs, $local, page_base);
        write_object_value(corner.i, corner.f, table.size[r], table.value[r]);
      }
    }
  } finally {
    restoring = false;
  }
  for (const base of object_columns()) reindex_chunk(base);
}

function load(): void {
  if (typeof localStorage === "undefined") return;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) decode_objects(raw);
  } catch {
    // An unreadable blob starts the world empty, as a missing one does.
    pxl().pxl_object_clear();
  }
}

function save(): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(STORAGE_KEY, encode_objects());
}

load();
// Initial index build — O(total objects), paid once at module load.
for (const base of object_columns()) reindex_chunk(base);

/** Chunks and bands currently holding objects. */
export function object_store_stats(): { chunks: number; bands: number; cells: number } {
  const p = pxl();
  const row = new Uint32Array(p.memory.buffer, p.pxl_object_stat(), 3);
  return { chunks: row[0], bands: row[1], cells: row[2] };
}

// ---- Mutations ----

/**
 * i/f is taken as-is — whatever camera/ray.ts's world_adjacent_corner
 * already computed (the same coordinate the hover preview and overlap
 * check used), never re-derived or re-snapped here. One source of truth
 * for where a placement goes, not a second one that quietly overrides it.
 *
 * The module writes the leaf, compacts the band and drops it — and the
 * chunk — once every leaf in it reads back VOID: a redundant place() and a
 * remove() both write VOID on purpose, and a record carrying no
 * information would otherwise sit in every save for nothing.
 */
function write_object_value(i: IVec, f: FVec, size: f64, val: number): void {
  const coord: PVF3 = { i, f };
  // size is always an exact power of two by construction (object/size.ts's
  // OBJECT_SIZES); Math.round only guards against float noise. The whole
  // cell is level 0, inside a band tree's range, so it needs no special
  // case the way a cell's own root once did.
  const level = Math.round(Math.log2(size));
  const at = lower(coord);
  const base = at.key;
  pxl().pxl_object_write(base, at.band, at.x, at.y, at.z, level, val);
  if (restoring) return;
  save();
  reindex_chunk(base);
}

/**
 * Whether painting `color_id` at this exact whole cell would actually
 * change anything a viewer could see — true only for a terrain-styled pick
 * (cell/mod.ts's own TERRAIN_STYLE_SOURCE) whose own channel+type exactly
 * matches what the REAL baked terrain already shows there. Painting
 * "land.GREEN_DEEP" over ground the bake already recorded as GREEN_DEEP
 * land is a no-op visually — the object layer would just be shadowing the
 * exact same information the terrain layer underneath it already carries,
 * permanently, for every future save/load of this cell. Scoped to whole-cell
 * writes only: a sub-cell pick paints a fraction of the cell, never fully
 * standing in for what the terrain layer shows across the whole thing, so
 * it can never be redundant by this same reasoning.
 *
 * Two things have to hold, and each was wrong on its own.
 *
 * The pick must match what the terrain actually *shows*, not merely a
 * channel it happens to carry — bake/ground.ts's shown_ground, the same
 * precedence the shader resolves a texture layer with. A road over bare
 * ground reads land=BARE and renders as road, so comparing the pick's own
 * channel alone cancelled a bare-ground cube into a road.
 *
 * And the cell must already hold an explicit AIR leaf, so the placement is
 * undoing a carve rather than adding a voxel terrain does not have. A
 * ground type is a per-column (2.5D) fact while a placement lands in the
 * empty cell above the surface, so without this every cube stacked on
 * matching ground read as redundant and was silently dropped.
 */
function place_value_is_redundant(i: IVec, f: FVec, size: f64, color_id: PlaceableCubeId, show_structures: boolean): boolean {
  if (size < U_PER_CELL) return false;
  if (!is_terrain_style_cube(color_id)) return false;

  const { channel, terrain_num_id } = terrain_style_cube_source(color_id);
  const shown = shown_ground((ch) => get_ground_type(i, f, ch) ?? 0, show_structures);
  if (shown.channel !== channel || shown.type_id !== terrain_num_id) return false;

  return object_value_is_air(read_state({ i, f }));
}

/**
 * Carving a cell (place_air) and then re-painting it with the exact
 * terrain-styled cube the real ground already is there cancels back out to
 * nothing, not to two stacked-but-inert entries: rather than storing a
 * "solid" leaf that only ever repaints what terrain would already show,
 * this writes VOID instead — the same absence remove() itself produces —
 * so write_object_value's own prune_if_empty can reclaim the cell entirely
 * the moment nothing else in it carries real information. A real color (or
 * a terrain-styled pick that doesn't match the real ground) still writes
 * normally.
 */
export function place(i: IVec, f: FVec, size: f64, color_id: PlaceableCubeId, show_structures: boolean = false): void {
  const val = place_value_is_redundant(i, f, size, color_id, show_structures) ? cube_types.common.VOID : placeable_cube_num_id(color_id);
  write_object_value(i, f, size, val);
}

/** Same write path as place(), just tagged with a decor kind instead of a
 * color — see decor.ts's own doc comment. Always the whole-cell tier in
 * practice today (mode_bar.tsx's decor buttons don't expose place()'s own
 * size flyout), but this doesn't assume that: any OBJECT_SIZES tier still
 * round-trips correctly through the exact same level/is_whole_cell logic a
 * color cube uses. */
export function place_decor(i: IVec, f: FVec, size: f64, decor_id: DecorId): void {
  write_object_value(i, f, size, decor_num_id(decor_id));
}

/** Eraser's own write path for its "nothing placed here" case
 * (camera/input.tsx's handle_tap) — a real, present leaf
 * (cube_types.common.AIR, object_value_is_air) that means "nothing here, on
 * purpose," unlike remove() below. remove() deletes a leaf outright and lets
 * whatever's underneath (almost always real baked terrain) show through
 * again; that's no help against terrain itself, which has no object-layer
 * leaf of its own to delete. Writing AIR instead overrides it regardless of
 * what's there — real terrain, or a placed cube/decor entry, all get
 * overwritten the same way write_object_value already overwrites any prior
 * value at this slot. */
export function place_air(i: IVec, f: FVec, size: f64): void {
  write_object_value(i, f, size, cube_types.common.AIR);
}

export function remove(id: string): void {
  const parsed = parse_leaf_id(id);
  if (!parsed) return;
  // The id carries its cell and its band-local position (leaf_id), which is
  // all a write back to VOID needs: the band's corner, then the offset.
  const [ix, iy, iz] = parse_cell_key(parsed.key);
  const corner = cell_corner(floor_div(ix, CHUNK_CELLS) * CHUNK_CELLS, floor_div(iy, CHUNK_CELLS) * CHUNK_CELLS, floor_div(iz, CHUNK_CELLS) * CHUNK_CELLS);
  const abs = pvf3.add({ i: 0n, f: 0n }, corner, local_offset(parsed.x, parsed.y, parsed.z));
  write_object_value(abs.i, abs.f, 2 ** parsed.level, VOID);
}
