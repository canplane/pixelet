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

import { type IVec, ivec, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { FROM_L3_IDX, L3_IDX, L3_RANGE } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import type { POI } from "@pxl/shared/bake/model/feature.ts";

import { with_ele } from "../terrain/elevation.ts";
import { FAR_END } from "../billboard/distance_fade.ts";
import { evict_poi_badges } from "./poi_badge.ts";
import { stable_hash } from "../billboard/batch.ts";

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
 * It is also not optional. `poi_map` is evicted at CLEANUP_RADIUS, so it
 * holds up to 7x7 = 49 pages, and sweeping all of them would put the whole
 * resident world through a structure whose capacity is meant to be constant.
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

const index = new SpatialHash(CAPACITY, BUCKETS, CELL);

/* Lanes, not records. Entry `i` is the same `i` across all of them and is
 * the id the spatial index was given, so a query answers in indices and
 * nothing between the store and the renderer is ever an object.
 *
 * The two reference lanes are the exception and cannot be otherwise: a POI
 * carries strings, and a page index is a BigInt. Everything a hot loop
 * touches is numeric, which is the part that matters — and the part that
 * crosses to linear memory unchanged. */
const poi_ref: (POI | null)[] = new Array(CAPACITY).fill(null);
const page_ref: (IVec | null)[] = new Array(CAPACITY).fill(null);
/** Flat anchor, absolute, split into its two BigInt lanes. Grounding is the
 * caller's, applied to the few a query returns. */
const abs_i = new BigInt64Array(CAPACITY);
const abs_f = new BigInt64Array(CAPACITY);
/** Local render-frame position, which is what was indexed — grounded, so
 * pos_z is the real terrain surface rather than the bake's flat zero. */
const pos_x = new Float64Array(CAPACITY);
const pos_y = new Float64Array(CAPACITY);
const pos_z = new Float64Array(CAPACITY);
/** 0 while this entry's terrain had not arrived when the index was built.
 * Indexed anyway so a query still finds it, but not drawable: the alternative
 * is drawing it at the bake's z=0, which is underground. */
const grounded_ok = new Uint8Array(CAPACITY);
/**
 * The POI's cell, as signed integers relative to the frame origin.
 *
 * This is the implicit tree. A node at tier k is `cell >> k` on each axis —
 * no node objects, no pointers, no traversal, which is 3D Tiles' own lesson
 * about implicit tiling: address by coordinate and build nothing. Every tier
 * is available from the same three lanes, so a tier change costs a shift
 * rather than a rebuild, and two badges at different distances can sit at
 * different tiers in the same frame. That last one is what a single tier per
 * frame cannot do and what perspective requires.
 *
 * Integers, not the float positions beside them. The frame origin is
 * L4-aligned (4096u) and every tier's span divides 4096, so these are exact
 * and their boundaries do not move when the origin does.
 */
const cell_x = new Int32Array(CAPACITY);
const cell_y = new Int32Array(CAPACITY);
const cell_z = new Int32Array(CAPACITY);

/** FNV-1a of the id, hashed once here rather than once per frame in the
 * selector that only ever uses it to break a tie. */
const tiebreak = new Float64Array(CAPACITY);

let count: u32 = 0;

/** What the current index was built from. Identity is enough for the first
 * two: the POI store replaces the map rather than mutating it, and the frame
 * origin is a BigInt that only moves on an L4 crossing. The centre is a
 * position, so it needs a distance instead — see REBUILD_MARGIN2. */
let built_from: ReadonlyMap<IVec, readonly POI[]> | null = null;
let built_origin: IVec | null = null;
let built_x: f64 = 0;
let built_y: f64 = 0;

export type PoiIndexStats = {
  indexed: u32;
  dropped: u32;
  rebuilds: u32;
};

let rebuilds: u32 = 0;
let dropped: u32 = 0;

const $seen = new Set<string>();
const $flat: PVF3 = { i: 0n, f: 0n };
const $ground: PVF3 = { i: 0n, f: 0n };

/**
 * Rebuilds if the page set or the frame origin changed, or if the camera has
 * left the disc the last build covers. Cheap enough to call every tick,
 * which is the point — the caller does not have to know when a page arrived.
 */
export function sync_poi_index(
  poi_map: ReadonlyMap<IVec, readonly POI[]>,
  map_center: PVF3,
  map_origin: IVec,
  to_local: (absolute: PVF3, origin: IVec) => readonly [f64, f64, f64],
  flat_pos: (out: PVF3, poi: POI, page_idx: IVec) => PVF3,
): void {
  // The centre page, and where the camera sits inside it. Everything below
  // works in these page-local real units rather than in PVF3: a page's own
  // POI already carry page-local centres, so the reach test is plain float
  // arithmetic and only what survives it pays for a BigInt.
  const center_page = TO_XY(L3_IDX(map_center.i));
  const [cx, cy] = to_local(map_center, FROM_L3_IDX(center_page));

  const moved_x = cx - built_x, moved_y = cy - built_y;
  const same_place = moved_x * moved_x + moved_y * moved_y <= REBUILD_MARGIN2;
  const pages_changed = poi_map !== built_from;
  if (!pages_changed && map_origin === built_origin && same_place) return;

  built_from = poi_map;
  built_origin = map_origin;
  built_x = cx;
  built_y = cy;
  rebuilds++;

  // The same id can arrive in two pages, since a place near a boundary is
  // baked into both. Deduping here rather than per tick is the whole
  // rearrangement: a Set once per rebuild instead of once per frame.
  $seen.clear();
  count = 0;
  dropped = 0;

  index.begin();
  for (let dy = -PAGE_RADIUS; dy <= PAGE_RADIUS; dy++) {
    for (let dx = -PAGE_RADIUS; dx <= PAGE_RADIUS; dx++) {
      const page_idx = TO_XY(ivec.add(center_page, ivec.gather(BigInt(dx), BigInt(dy), 0n)));
      const pois = poi_map.get(page_idx);
      if (pois === undefined) continue;

      // The camera, expressed in this page's own local frame.
      const px = cx - dx * PAGE_U, py = cy - dy * PAGE_U;

      for (const poi of pois) {
        // Horizontal only, and deliberately: this runs before grounding, so
        // the only z available is the bake's flat zero while the camera's is
        // a real elevation. A cylinder contains the sphere the exact test
        // uses, so admitting a few extra costs a terrain lookup each and
        // rejects nothing that should have been kept. The exact, isotropic
        // test is the caller's, on positions that have a real z by then.
        const ox = poi.center[0] - px, oy = poi.center[1] - py;
        if (ox * ox + oy * oy > RESIDENCY2) continue;
        if ($seen.has(poi.id)) continue;
        $seen.add(poi.id);
        if (count >= CAPACITY) {
          dropped++;
          continue;
        }

        // Grounded here rather than per tick, which is the whole point of
        // doing it at build time: a terrain lookup for everything in the disc
        // once every few hundred metres of travel, instead of one for every
        // candidate a query returns, five times a second.
        //
        // It is also what makes the index isotropic. The query is a sphere in
        // xyz (spatial_hash.ts), and map_center carries the terrain height
        // under it (use_ground_snap_sync.ts) — so indexing at the bake's flat
        // z put every POI a full elevation away from the point being asked
        // about, and above CELL the z cells stopped overlapping at all.
        const flat = flat_pos($flat, poi, page_idx);
        const grounded = with_ele($ground, flat.i, flat.f);

        // Not snapped here. Which cell a badge stands in depends on its tier
        // and its tier depends on its distance, so the snap belongs to the
        // tick — see the cell lanes above. What is stored is the place
        // itself, grounded.
        const absolute = grounded ?? flat;
        const [x, y, z] = to_local(absolute, map_origin);
        if (!index.insert(count, x, y, z)) {
          dropped++;
          continue;
        }

        const i = count++;
        poi_ref[i] = poi;
        page_ref[i] = page_idx;
        abs_i[i] = absolute.i;
        abs_f[i] = absolute.f;
        pos_x[i] = x;
        pos_y[i] = y;
        pos_z[i] = z;
        // Exact: `absolute.i` and `map_origin` are both integer lattice
        // addresses, so this is a lattice difference and not a rounded one.
        cell_x[i] = Number(ivec.X(absolute.i) - ivec.X(map_origin));
        cell_y[i] = Number(ivec.Y(absolute.i) - ivec.Y(map_origin));
        cell_z[i] = Number(ivec.Z(absolute.i) - ivec.Z(map_origin));
        grounded_ok[i] = grounded === null ? 0 : 1;
        tiebreak[i] = stable_hash(poi.id);
      }
    }
  }
  index.build();

  // Only on a page change, not on every step the camera takes. A badge just
  // outside the disc is one the camera is walking towards, and evicting it
  // on each rebuild would rebuild its canvas on the way back — a page
  // leaving is the event that actually makes one unreachable.
  if (pages_changed) evict_poi_badges($seen);
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
  return index.query(x, y, z, radius, out);
}

/* Lane accessors. Deliberately plain functions rather than a returned object:
 * a caller that wanted `{poi, x, y}` back would put the allocation this file
 * exists to remove straight back in. */
export function poi_at(i: u32): POI {
  return poi_ref[i]!;
}
export function poi_page_at(i: u32): IVec {
  return page_ref[i]!;
}
export function poi_abs_i(i: u32): bigint {
  return abs_i[i];
}
export function poi_abs_f(i: u32): bigint {
  return abs_f[i];
}
export function poi_x(i: u32): f64 {
  return pos_x[i];
}
export function poi_y(i: u32): f64 {
  return pos_y[i];
}
export function poi_z(i: u32): f64 {
  return pos_z[i];
}
export function poi_tiebreak(i: u32): f64 {
  return tiebreak[i];
}
export function poi_cell_x(i: u32): i32 {
  return cell_x[i];
}
export function poi_cell_y(i: u32): i32 {
  return cell_y[i];
}
export function poi_cell_z(i: u32): i32 {
  return cell_z[i];
}
export function poi_grounded(i: u32): boolean {
  return grounded_ok[i] !== 0;
}

/**
 * Forces the next sync to rebuild.
 *
 * The trigger this exists for is terrain arriving: elevation is resolved at
 * build time now, so an entry indexed before its tile landed carries the flat
 * z and is held back from drawing until a rebuild grounds it.
 */
export function invalidate_poi_index(): void {
  built_from = null;
}

export function poi_index_stats(): PoiIndexStats {
  return { indexed: count, dropped, rebuilds };
}
