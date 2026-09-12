// @pxl/web/modules/world/poi/poi_index.ts

/**
 * The loaded POI set, indexed by where it is, so a camera tick asks a
 * question instead of reading everything.
 *
 * What this replaces walked every POI in every page within
 * NEARBY_PAGE_RADIUS on every camera tick — roughly four thousand at the
 * measured load — allocating a `Set` for the id dedupe and one object per
 * candidate as it went, then grounding and sorting the result. The badges
 * that survive are capped at MAX_RENDERED, so almost all of that work was
 * spent on things that were never going to be drawn.
 *
 * POI is static. It changes when a page arrives or leaves and at no other
 * time, which is the axis this indexes along: build once per page set, and
 * per tick do one radius query into a buffer that already exists. That is
 * the store/index split
 * [discussions/20260910-storage-model-precedents.md] arrives at — the store
 * partitions by what does not move, the index by where things are.
 *
 * Positions are held in the local render frame rather than as PVF3, because
 * that is the frame the query is asked in and converting eight thousand
 * BigInt pairs per tick is the cost being removed. The frame's origin is
 * L4-aligned and so only shifts every 4096u; when it does, this rebuilds,
 * which is the same trigger a page change already is.
 *
 * Grounded positions, resolved once at build time. The reach test is a
 * sphere and map_center sits on the terrain surface, so an entry held at the
 * bake's flat z is an entry the query cannot find on a mountain — and doing
 * the lookup here costs one pass over the disc per rebuild instead of one per
 * candidate per tick, which was the larger of the two costs anyway.
 */

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";
import { SpatialHash } from "@xpute/core/ds/spatial_hash.ts";
import type { FrameBudget } from "@/modules/world/sched/tick.ts";

import { type IVec, ivec, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { FROM_L3_IDX, L3_IDX, L3_RANGE } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import type { PoiPage } from "@pxl/shared/bake/format.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/ds/map.ts";

import { with_ele } from "../terrain/elevation.ts";
import { FAR_END } from "../billboard/distance_fade.ts";
import { evict_poi_badges } from "./poi_badge.ts";
import { poi_pages } from "./store.ts";
import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

/**
 * How far out this keeps POI, as a multiple of the query reach.
 *
 * The index holds a disc rather than the whole resident page set, and this
 * is what makes a fixed capacity a statement about geometry instead of a
 * hope about the data. A query is valid as long as everything within
 * FAR_END of the camera is present, so a disc of RESIDENCY around the point
 * it was built at stays valid until the camera moves REBUILD_MARGIN — which
 * is where the rebuild trigger below comes from.
 *
 * Two rather than one because the margin is what buys the rebuilds being
 * rare; larger costs capacity quadratically, and 640u already covers 605 m.
 */
const RESIDENCY: f64 = FAR_END * 2;
const RESIDENCY2: f64 = RESIDENCY * RESIDENCY;
const REBUILD_MARGIN2: f64 = (RESIDENCY - FAR_END) * (RESIDENCY - FAR_END);

/**
 * Pages to sweep around the centre one. RESIDENCY (640u) is under a page
 * (L3_RANGE, 1024u), so a point inside the centre page cannot reach past an
 * immediate neighbour — this is exact rather than a heuristic.
 *
 * It is also not optional. `poi_pages` holds the nearest RESIDENT_PAGES
 * (terrain/config.ts), 32 of them, and sweeping all of them would put the
 * whole resident world through a structure whose capacity is meant to be
 * constant.
 */
const PAGE_RADIUS = 1;
const PAGE_U: f64 = Number(L3_RANGE);

/**
 * Capacity is the design, not a guess at the data: a fixed ceiling is what
 * keeps the live object count independent of how much world has been
 * visited. Overflow is reported through `stats()` rather than absorbed.
 *
 * The number comes from the densest page in the current bake — 12544 POI in
 * one 1024u page, so 0.012 per u2 — which puts a RESIDENCY disc at 15.4k.
 * This is the next power of two above that. The same density over the 3x3
 * pages would be 113k, which is what the disc is for.
 */
const CAPACITY: u32 = 32768;

/** Largest prime below 2^16, above CAPACITY. Teschner measured the curve
 * flattening well before the table reaches the item count, and the cost of
 * overshooting is memory rather than time. */
const BUCKETS: u32 = 65521;

/**
 * The cell is the query radius, which here is the reach the old scan used
 * as its coarse limit. One cell per radius keeps a query to a handful of
 * buckets and a POI in exactly one — Teschner measured that ratio as the
 * optimum and found cell size to matter more than table size.
 */
const CELL: f64 = FAR_END;

/**
 * One set of lanes and the hash over them. Two exist: the one queries read
 * and the one a rebuild fills. A rebuild is swept a page per step from the
 * frame's content phase, so it can span several frames — and while it does,
 * the previous build keeps answering. Nothing ever queries a half-built
 * table, and nothing has to finish inside one frame to keep the map drawn.
 *
 * Entry `i` is the same `i` across every lane and is the id the spatial
 * index was given, so a query answers in indices and nothing between the
 * store and the renderer is ever an object. A record is named by its page
 * and its index in that page's lanes; the page handle is the one reference
 * lane, since a page is a packet. Everything a hot loop touches is numeric.
 */
type Lanes = {
  index: SpatialHash;
  page_ref: (PoiPage | null)[];
  page_idx: BigUint64Array;
  rec: Uint32Array;
  /** Flat anchor, absolute, split into its two BigInt lanes. */
  abs_i: BigInt64Array;
  abs_f: BigInt64Array;
  /** Local render-frame position, which is what was indexed — grounded, so
   * pos_z is the real terrain surface rather than the bake's flat zero. */
  pos_x: Float64Array;
  pos_y: Float64Array;
  pos_z: Float64Array;
  /** 0 while this entry's terrain had not arrived when it was indexed.
   * Indexed anyway so a query still finds it, but not drawable: the
   * alternative is drawing it at the bake's z=0, which is underground. */
  grounded_ok: Uint8Array;
  /**
   * The POI's cell, as signed integers relative to the frame origin.
   *
   * This is the implicit tree. A node at tier k is `cell >> k` on each axis —
   * no node objects, no pointers, no traversal, which is 3D Tiles' own lesson
   * about implicit tiling: address by coordinate and build nothing. Every
   * tier is available from the same three lanes, so a tier change costs a
   * shift rather than a rebuild, and two badges at different distances can
   * sit at different tiers in the same frame.
   *
   * Integers, not the float positions beside them. The frame origin is
   * L4-aligned (4096u) and every tier's span divides 4096, so these are exact
   * and their boundaries do not move when the origin does.
   */
  cell_x: Int32Array;
  cell_y: Int32Array;
  cell_z: Int32Array;
  /** FNV-1a of the id, hashed once here rather than once per frame in the
   * selector that only ever uses it to break a tie. */
  tiebreak: Float64Array;
  count: u32;
  dropped: u32;
};

function make_lanes(): Lanes {
  return {
    index: new SpatialHash(CAPACITY, BUCKETS, CELL, CORE_ALLOC),
    page_ref: new Array(CAPACITY).fill(null),
    page_idx: CORE_ALLOC.take(BigUint64Array, CAPACITY),
    rec: CORE_ALLOC.take(Uint32Array, CAPACITY),
    abs_i: CORE_ALLOC.take(BigInt64Array, CAPACITY),
    abs_f: CORE_ALLOC.take(BigInt64Array, CAPACITY),
    pos_x: CORE_ALLOC.take(Float64Array, CAPACITY),
    pos_y: CORE_ALLOC.take(Float64Array, CAPACITY),
    pos_z: CORE_ALLOC.take(Float64Array, CAPACITY),
    grounded_ok: CORE_ALLOC.take(Uint8Array, CAPACITY),
    cell_x: CORE_ALLOC.take(Int32Array, CAPACITY),
    cell_y: CORE_ALLOC.take(Int32Array, CAPACITY),
    cell_z: CORE_ALLOC.take(Int32Array, CAPACITY),
    tiebreak: CORE_ALLOC.take(Float64Array, CAPACITY),
    count: 0,
    dropped: 0,
  };
}

let live: Lanes = make_lanes();
let back: Lanes = make_lanes();

/** What `live` was built from. A number is enough for the first two: the
 * page table's revision moves on every page set or cleared, and the frame
 * origin is a BigInt that only moves on an L4 crossing. The centre is a
 * position, so it needs a distance instead — see REBUILD_MARGIN2. -1 before
 * any build, and again when the index is invalidated. */
let built_from: i32 = -1;
let built_origin: IVec | null = null;
let built_x: f64 = 0;
let built_y: f64 = 0;

/**
 * A rebuild in progress: everything the sweep needs, and how far it got.
 * `pages` is the 3x3 around the centre in sweep order; `next` is the first
 * one not yet done.
 */
type Job = {
  /** poi_pages.revision the sweep reads at. */
  pages_revision: u32;
  map_origin: IVec;
  cx: f64;
  cy: f64;
  pages: { page_idx: IVec; dx: number; dy: number }[];
  next: u32;
  pages_changed: boolean;
  to_local: (absolute: PVF3, origin: IVec) => readonly [f64, f64, f64];
  flat_pos: (out: PVF3, page: PoiPage, rec: u32, page_idx: IVec) => PVF3;
};
let job: Job | null = null;

export type PoiIndexStats = {
  indexed: u32;
  dropped: u32;
  rebuilds: u32;
  /** Pages the current rebuild has left, 0 when none is running. */
  sweeping: u32;
};

let rebuilds: u32 = 0;

/** The ids seen this rebuild, by their 64-bit hash — a place near a page
 * boundary is baked into both pages. What eviction asks against, too. */
const $seen = new IndexedMap(I64_KEYS, 1 << 12, CORE_ALLOC);
const $flat: PVF3 = { i: 0n, f: 0n };
const $ground: PVF3 = { i: 0n, f: 0n };

/**
 * Asks for a rebuild if the page set or the frame origin changed, or if the
 * camera has left the disc the live build covers. Cheap enough to call every
 * tick — the caller does not have to know when a page arrived.
 *
 * It only *asks*. The rebuild itself runs in `poi_index_sweep`, a page per
 * step under the frame's budget, and the live index keeps answering until
 * the new one is whole. Returns true when the live index already reflects
 * these inputs.
 *
 * A job already running is replaced only when the page set or the origin
 * moved — a camera that keeps walking during a rebuild would otherwise
 * restart it every frame and never finish. Its centre is at most a margin
 * behind, which the disc was sized to absorb.
 */
export function sync_poi_index(
  map_center: PVF3,
  map_origin: IVec,
  to_local: (absolute: PVF3, origin: IVec) => readonly [f64, f64, f64],
  flat_pos: (out: PVF3, page: PoiPage, rec: u32, page_idx: IVec) => PVF3,
): boolean {
  // The centre page, and where the camera sits inside it. Everything works
  // in these page-local real units rather than in PVF3: a page's own POI
  // already carry page-local centres, so the reach test is plain float
  // arithmetic and only what survives it pays for a BigInt.
  const center_page = TO_XY(L3_IDX(map_center.i));
  const [cx, cy] = to_local(map_center, FROM_L3_IDX(center_page));

  const moved_x = cx - built_x, moved_y = cy - built_y;
  const same_place = moved_x * moved_x + moved_y * moved_y <= REBUILD_MARGIN2;
  const pages_revision = poi_pages.revision;
  const pages_changed = pages_revision !== built_from;
  if (!pages_changed && map_origin === built_origin && same_place) return true;

  if (job && job.pages_revision === pages_revision && job.map_origin === map_origin) return false;

  const pages: Job["pages"] = [];
  for (let dy = -PAGE_RADIUS; dy <= PAGE_RADIUS; dy++) {
    for (let dx = -PAGE_RADIUS; dx <= PAGE_RADIUS; dx++) {
      pages.push({ page_idx: TO_XY(ivec.add(center_page, ivec.gather(BigInt(dx), BigInt(dy), 0n))), dx, dy });
    }
  }
  // Nearest first, by distance from the target to each page's rectangle —
  // the same "from where the view is heading" rule as everything else the
  // tick schedules (sched/priority.ts). The table only goes live whole, so
  // this does not change what is drawn; it changes which pages a sweep cut
  // short by a job restart has already paid for.
  const page_gap2 = (p: Job["pages"][number]): f64 => {
    const x0 = p.dx * PAGE_U, y0 = p.dy * PAGE_U;
    const gx = cx < x0 ? x0 - cx : cx > x0 + PAGE_U ? cx - x0 - PAGE_U : 0;
    const gy = cy < y0 ? y0 - cy : cy > y0 + PAGE_U ? cy - y0 - PAGE_U : 0;
    return gx * gx + gy * gy;
  };
  pages.sort((a, b) => page_gap2(a) - page_gap2(b));
  job = { pages_revision, map_origin, cx, cy, pages, next: 0, pages_changed, to_local, flat_pos };

  // The same id can arrive in two pages, since a place near a boundary is
  // baked into both. Deduping once per rebuild rather than per tick is the
  // whole rearrangement.
  $seen.clear();
  back.count = 0;
  back.dropped = 0;
  back.index.begin();
  return false;
}

/** One page of the sweep, into `back`. */
function sweep_page(j: Job, page: Job["pages"][number]): void {
  const resident = poi_pages.get(page.page_idx);
  if (resident === undefined) return;
  const pois = resident.page;
  const lanes = back;

  // The camera, expressed in this page's own local frame.
  const px = j.cx - page.dx * PAGE_U, py = j.cy - page.dy * PAGE_U;

  for (let r = 0; r < pois.len; r++) {
    // Horizontal only, and deliberately: this runs before grounding, so the
    // only z available is the bake's flat zero while the camera's is a real
    // elevation. A cylinder contains the sphere the exact test uses, so
    // admitting a few extra costs a terrain lookup each and rejects nothing
    // that should have been kept. The exact, isotropic test is the caller's,
    // on positions that have a real z by then.
    const ox = pois.x[r] - px, oy = pois.y[r] - py;
    if (ox * ox + oy * oy > RESIDENCY2) continue;
    const id_hash = pois.id_hash[r];
    if ($seen.find(id_hash) >= 0) continue;
    $seen.insert(id_hash);
    if (lanes.count >= CAPACITY) {
      lanes.dropped++;
      continue;
    }

    // Grounded here rather than per tick, which is the whole point of doing
    // it at build time: a terrain lookup for everything in the disc once
    // every few hundred metres of travel, instead of one for every candidate
    // a query returns, five times a second.
    //
    // It is also what makes the index isotropic. The query is a sphere in
    // xyz (spatial_hash.ts), and map_center carries the terrain height under
    // it (use_ground_snap_sync.ts) — so indexing at the bake's flat z put
    // every POI a full elevation away from the point being asked about, and
    // above CELL the z cells stopped overlapping at all.
    const flat = j.flat_pos($flat, pois, r, page.page_idx);
    const grounded = with_ele($ground, flat.i, flat.f);

    // Not snapped here. Which cell a badge stands in depends on its tier and
    // its tier depends on its distance, so the snap belongs to the tick —
    // see the cell lanes above. What is stored is the place itself, grounded.
    const absolute = grounded ?? flat;
    const [x, y, z] = j.to_local(absolute, j.map_origin);
    if (!lanes.index.insert(lanes.count, x, y, z)) {
      lanes.dropped++;
      continue;
    }

    const i = lanes.count++;
    lanes.page_ref[i] = pois;
    lanes.page_idx[i] = page.page_idx;
    lanes.rec[i] = r;
    lanes.abs_i[i] = absolute.i;
    lanes.abs_f[i] = absolute.f;
    lanes.pos_x[i] = x;
    lanes.pos_y[i] = y;
    lanes.pos_z[i] = z;
    // Exact: `absolute.i` and `map_origin` are both integer lattice
    // addresses, so this is a lattice difference and not a rounded one.
    lanes.cell_x[i] = Number(ivec.X(absolute.i) - ivec.X(j.map_origin));
    lanes.cell_y[i] = Number(ivec.Y(absolute.i) - ivec.Y(j.map_origin));
    lanes.cell_z[i] = Number(ivec.Z(absolute.i) - ivec.Z(j.map_origin));
    lanes.grounded_ok[i] = grounded === null ? 0 : 1;
    // The hash's low 32 bits as a fraction: a stable, even tie-break.
    lanes.tiebreak[i] = Number(id_hash & 0xffffffffn) / 0x1_0000_0000;
  }
}

/**
 * Advances the rebuild, a page per step, for as long as the budget allows.
 * Returns true on the step that finishes it — the moment the live index
 * changed and whoever placed against the old one should place again.
 *
 * The swap is the only point at which queries see anything new, and it is
 * whole: the table is built, the lanes are complete, and the previous build
 * becomes the next rebuild's scratch.
 */
export function poi_index_sweep(budget: FrameBudget): boolean {
  const j = job;
  if (!j) return false;

  budget.run(j.pages.slice(j.next), (page) => {
    sweep_page(j, page);
    j.next++;
    return true;
  });
  if (j.next < j.pages.length) return false;

  back.index.build();
  const finished = back;
  back = live;
  live = finished;
  built_from = j.pages_revision;
  built_origin = j.map_origin;
  built_x = j.cx;
  built_y = j.cy;
  rebuilds++;
  job = null;

  // Only on a page change, not on every step the camera takes. A badge just
  // outside the disc is one the camera is walking towards, and evicting it
  // on each rebuild would rebuild its canvas on the way back — a page
  // leaving is the event that actually makes one unreachable.
  if (j.pages_changed) evict_poi_badges($seen);
  return true;
}

/**
 * Every indexed POI whose cell is within `radius` of a local-frame point, as
 * indices written into `out`.
 *
 * Broadphase, so the caller still applies its own exact test — the index
 * answers in cells and the reach limit is a circle. Indices rather than
 * records, so nothing is built to be thrown away: the caller reads the lanes
 * it needs through the accessors below.
 */
export function query_poi_index(x: f64, y: f64, z: f64, radius: f64, out: Uint32Array): u32 {
  return live.index.query(x, y, z, radius, out);
}

/* Lane accessors. Deliberately plain functions rather than a returned object:
 * a caller that wanted `{poi, x, y}` back would put the allocation this file
 * exists to remove straight back in. A record is its page and its index
 * there; the page's own lanes and strings are read through those. */
export function poi_page(i: u32): PoiPage {
  return live.page_ref[i]!;
}
export function poi_rec(i: u32): u32 {
  return live.rec[i];
}
export function poi_page_at(i: u32): IVec {
  return live.page_idx[i];
}
export function poi_abs_i(i: u32): bigint {
  return live.abs_i[i];
}
export function poi_abs_f(i: u32): bigint {
  return live.abs_f[i];
}
export function poi_x(i: u32): f64 {
  return live.pos_x[i];
}
export function poi_y(i: u32): f64 {
  return live.pos_y[i];
}
export function poi_z(i: u32): f64 {
  return live.pos_z[i];
}
export function poi_tiebreak(i: u32): f64 {
  return live.tiebreak[i];
}
export function poi_cell_x(i: u32): i32 {
  return live.cell_x[i];
}
export function poi_cell_y(i: u32): i32 {
  return live.cell_y[i];
}
export function poi_cell_z(i: u32): i32 {
  return live.cell_z[i];
}
export function poi_grounded(i: u32): boolean {
  return live.grounded_ok[i] !== 0;
}

/**
 * Forces the next sync to rebuild.
 *
 * The trigger this exists for is terrain arriving: elevation is resolved at
 * build time now, so an entry indexed before its tile landed carries the flat
 * z and is held back from drawing until a rebuild grounds it.
 */
export function invalidate_poi_index(): void {
  built_from = -1;
}

export function poi_index_stats(): PoiIndexStats {
  return { indexed: live.count, dropped: live.dropped, rebuilds, sweeping: job ? job.pages.length - job.next : 0 };
}
