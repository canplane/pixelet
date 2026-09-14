// @pxl/web/module/world/poi/poi_layer.tsx

/**
 * POI's own render — world-anchored-billboard tier (see
 * issue/20260907-world-rendering-tiers.md), a billboard set of the module
 * (crates/pxl-core/src/world/billboard.rs) beside chat_bubble.tsx's. A category-themed badge
 * (poi_theme.ts), brought back from the pre-DoD implementation
 * (`checkpoint/20260107-float`'s `entities/geo/ui/pois/poi-item.tsx`) —
 * still deliberately short of that version in a few ways:
 *
 * - No click-to-select (`map/store.ts`'s `selected_poi` exists but nothing
 *   writes to it), no hover state (that version's own lift/glow/expand-on-
 *   hover needs raycasting against sprites, not wired here).
 * - No day/night theme variant on the badge itself.
 * - No two-tier LOD: that version rendered its FULL dataset (10,000+) as a
 *   cheap custom-shader instanced-dot background layer, promoting only the
 *   nearest ~20 to the expensive per-item treatment every 0.2s. This file
 *   instead picks up to MAX_RENDERED (see select_nearby_pois below — real
 *   camera-frustum membership first, a stable per-id cut if still over
 *   budget) and renders exactly those. Fine at today's scale; revisit if it
 *   stops being fine.
 */

import { useEffect } from "react";
import { request_world_tick as invalidate } from "@/module/world/change.ts";

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";
import { FROM_L3_IDX, SPAN_CENTER } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { PoiPage } from "@pxl/shared/bake/format.ts";

import { useMapStore } from "@/module/map/store.ts";
import { useCameraStore } from "@/module/camera/store.ts";
import { useSettingsStore } from "@/app/setting/store.ts";
import { get_local_world_origin, local_world_to_canonical } from "../frame/coord.ts";
import { register_tile_interest } from "../terrain/store.ts";
import { BILLBOARD_ORDER } from "../billboard/sheet.ts";
import { select_begin, select_place, select_remaining } from "../billboard/frustum_budget.ts";
import { BillboardBatch } from "../billboard/batch.ts";
import { CellNodes } from "../billboard/cell_node.ts";
import {
  invalidate_poi_index,
  NO_TIER,
  poi_cell_x,
  poi_cell_y,
  poi_cell_z,
  poi_grounded,
  poi_index_stats,
  poi_index_sweep,
  poi_page,
  poi_rec,
  poi_tiebreak,
  poi_tier,
  poi_x,
  poi_y,
  poi_z,
  query_poi_index,
  set_poi_tier,
  sync_poi_index,
} from "./poi_index.ts";
import { badge_atlas, BADGE_CENTER_Y, BADGE_COLLIDE_H, BADGE_LOGICAL_H, cluster_badge_logical_w, cluster_badge_slot, poi_badge_logical_w, poi_badge_slot } from "./poi_badge.ts";
import { NO_SLOT } from "../billboard/atlas.ts";
import { BADGE_SHAMT_OVER_CUBE, fade_end_u, get_lod_level, lod_step_at, LOD_STEP_BY_LEVEL, type LodStep } from "@/render/lod.ts";
import { usePoiStore } from "./store.ts";
import { type FrameBudget, useTick } from "@/module/world/sched/tick.ts";
import { CORE_ALLOC, CORE_PXL, FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";
import { view } from "@/module/world/view.ts";

/**
 * A rail, not the mechanism.
 *
 * What decides how many badges are drawn is how many fit: a candidate that
 * lands on pixels another badge has taken is folded into that badge's count
 * (frustum_budget.ts), so density answers itself and the number left standing
 * is whatever the screen has room for. This only bounds the sprite and
 * draw-call count if something goes wrong, and it is set high enough that a
 * full screen of non-overlapping badges never reaches it.
 *
 * It used to be 80, and it used to be the whole answer — four POI in one
 * building meant one drawn and three deleted, with nothing recording that
 * they had ever been there. Aggregation is what a budget should buy; deletion
 * is what it bought.
 */
const MAX_RENDERED = 512;

/** The finest and coarsest cell a badge is allowed to snap to, as shifts of
 * the canonical unit. 0 is 1u, the terrain cell itself; 6 is 64u, which is
 * an L2 chunk and the top of the ladder terrain and decor already climb
 * (render/lod.ts's 1/2/4/.../64). Beyond that the tier belongs to the page,
 * not to the badge. */
const MIN_SHAMT: u32 = 0;
const MAX_SHAMT: u32 = 6;

/**
 * Which lattice tier a badge snaps to: the cube tier at its range, by the
 * same criterion and the same hysteresis chunks use (lod_step_at), raised
 * BADGE_SHAMT_OVER_CUBE tiers. So a badge gathers or spreads at exactly the
 * distance the terrain under that range hands off, and resists flipping
 * back the way a chunk does. The level it last showed is its record's
 * one-byte state (poi_index.ts).
 */
function tier_shamt(dist: f64, i: u32): u32 {
  const last = poi_tier(i);
  const current: LodStep = last === NO_TIER ? LOD_STEP_BY_LEVEL[LOD_STEP_BY_LEVEL.length - 1] : LOD_STEP_BY_LEVEL[last];
  const level = get_lod_level(lod_step_at(Math.max(1e-3, dist), current));
  if (level !== last) set_poi_tier(i, level);
  return Math.max(MIN_SHAMT, Math.min(MAX_SHAMT, level + BADGE_SHAMT_OVER_CUBE)) as u32;
}

/** center.xyz (page-local real units, overture_provider.ts) resolved against
 * the page's own global corner, untouched. The index grounds it and then
 * snaps it to a cell centre, so nothing here adds a half of its own — that
 * used to be a lone `+ 0.5` on Z, which is the right number at exactly one
 * tier and on exactly one axis.
 *
 * Handed to the index as a callback rather than imported there, so the index
 * does not have to know how a POI is addressed. */
function poi_flat_pos(out: PVF3, page: PoiPage, rec: u32, page_idx: IVec): PVF3 {
  $center[0] = page.x[rec];
  $center[1] = page.y[rec];
  $center[2] = page.z[rec];
  return local_world_to_canonical(out, $center, FROM_L3_IDX(page_idx));
}
const $center: [f64, f64, f64] = [0, 0, 0];

/** The reach, squared. It only rules out what is too far to ever draw —
 * which of the survivors actually draws is select_visible_billboards' job
 * (frustum membership, then nearest-first greedy placement against the
 * screen), and never poi_extractor.ts's rank field: rank is "how notable is
 * this in general", not "is this visible right now". */

/**
 * Everything a tick works in, allocated once.
 *
 * `$hits` is sized for the query's *box*, not its circle. The index cell is
 * FAR_END, so a radius-FAR_END query touches three cells on each axis and
 * reports a 960u box before this file narrows it back to the circle. The
 * densest page in the bake is 12544 POI in one 1024u page, which puts that
 * box at ~11k, so this is the next power of two above it — a query that
 * filled the buffer would truncate in cell order, which is unrelated to
 * distance, and drop near badges to keep far ones.
 *
 * The batch grows instead, since what survives the circle is a quarter of
 * that and a frame's candidates are not a residency set (batch.ts).
 */
const $hits = CORE_ALLOC.take(Uint32Array, 16384);
const $batch = new BillboardBatch();
const $nodes = new CellNodes(4096, CORE_ALLOC);
/** The camera in the frame's own coordinates, as an address. */
const $cam_v3: [f64, f64, f64] = [0, 0, 0];
const $centre: PVF3 = { i: 0n, f: 0n };
const $kept = CORE_ALLOC.take(Uint32Array, MAX_RENDERED);

/** absolute, projected into local_world_frame's own origin (map_origin) —
 * the frame select_visible_billboards' local_x/y/z expects (see
 * AnchorNode's own local_pos in frame/coord.ts). */
function local_pos_from_absolute(absolute: PVF3, map_origin: IVec): [f64, f64, f64] {
  const local: PVF3 = { i: ivec.sub(absolute.i, map_origin), f: absolute.f };
  return [pvf3.to_real(pvf3.X(local)), pvf3.to_real(pvf3.Y(local)), pvf3.to_real(pvf3.Z(local))];
}

/**
 * What the last placement run cost and how much it threw away, for the dev
 * ledger. Not DEV-gated: two `performance.now()` calls at the placement rate
 * are free, and instrumentation that only exists in dev is instrumentation
 * that cannot answer a question about production.
 *
 * `hits` is what the spatial query returned, `candidates` what survived the
 * reach circle and got grounded, `kept` what is mounted. The gap between the
 * middle two is the per-candidate terrain work, which is what clustering
 * would be for.
 */
export const poi_place_stats = {
  ms: 0,
  max_ms: 0,
  sync_ms: 0,
  max_sync_ms: 0,
  runs: 0,
  hits: 0,
  max_hits: 0,
  candidates: 0,
  kept: 0,
  absorbed: 0,
  biggest: 0,
  /** Ask to commit, and the frames the job spanned: the starvation
   * question for a content step, the way `oldest_visible_ms` is the
   * chunks'. */
  wait_ms: 0,
  max_wait_ms: 0,
  frames: 0,
  max_frames: 0,
};

/** What the layer hands React. The anchor comes from the index rather than
 * being resolved again per badge: it is already grounded there, and one
 * rebuild replaces what used to be MAX_RENDERED separate terrain
 * subscriptions. */
type PoiPick = {
  /** The anchor in the render frame, which is what the field writes into a
   * slot. The absolute address is still resolved above it, because that is
   * where the lattice lives and where the identity comes from — this is the
   * one conversion, done once per drawn badge. */
  local_x: f64;
  local_y: f64;
  local_z: f64;
  /** The lattice node this badge is, as tier and *absolute* address. It is
   * the badge's identity: stable while the cell is populated at that tier,
   * and unmoved by the camera, by which member happens to be nearest, or by
   * the frame origin. Keying on a POI instead meant a new sprite, material
   * and CanvasTexture every time the nearest one changed, which is a
   * browser-owned object created and destroyed per camera nudge. */
  key: string;
  /** The place the badge names: its page and its record there. */
  page: PoiPage;
  rec: u32;
  anchor_i: IVec;
  anchor_f: bigint;
  /** How many places this badge stands for, itself included. Above one it is
   * drawn as a count rather than as any of them: nothing here is a
   * representative, and `poi` survives only to key the list. */
  members: u32;
  /** Filled by the build stage: the sheet rectangle and the drawn width. */
  slot: i32;
  logical_w: f64;
};

/**
 * Placement as a job of the frame's content phase.
 *
 * It was one synchronous pass in a React effect: 2 ms on most runs and
 * 180 ms on a phone over a cold index, none of it under the budget and all
 * of it booked by the ledger as off-thread, since it ran outside the tick
 * (issue/20260909-cooperative-scheduler-and-task-ledger.md, the
 * 2026-09-13 update). The same pass now runs as stages of bounded units
 * under `budget.run`, and a frame that cannot afford the whole of it does
 * what it can and comes back: query, then file the hits into lattice nodes
 * a thousand at a time, measure and offer the nodes a few hundred at a
 * time, select, build the badges a few at a time, and commit the field in
 * one step so the screen only ever shows a whole placement.
 *
 * A stage's cursor is into scratch the previous stage filled, so nothing is
 * rebuilt between frames — but a query answers in indices into the live
 * index, and a rebuild that finishes mid-job swaps that index out from
 * under them. The job checks the rebuild count and starts over when it
 * moved, rather than reading stale lanes.
 */
const QUERY = 0, FILE = 1, OFFER = 2, SELECT = 3, PLACE = 4, BUILD = 5, COMMIT = 6;
/** Units, sized so the slowest of each fits a small budget a few times
 * over on a phone: filing is float arithmetic, an offer is a width
 * estimate, a placement is a projection and a grid probe, a build is a
 * canvas draw. */
const FILE_UNIT: u32 = 1024;
const OFFER_UNIT: u32 = 256;
const PLACE_UNIT: u32 = 128;
const BUILD_UNIT: u32 = 4;

type PlaceJob = {
  stage: number;
  t0: f64;
  wanted_at: f64;
  frames: u32;
  /** The rebuild count the hits were queried against. */
  rebuilds: u32;
  map_origin: IVec;
  cx: f64;
  cy: f64;
  cz: f64;
  sync_ms: f64;
  hits: u32;
  /** Cursors: into $hits, into $nodes, into $kept. */
  h: u32;
  n: u32;
  kept: u32;
  /** How far the greedy placement has walked the sorted candidates. */
  placed: u32;
  k: u32;
  picks: PoiPick[];
  absorbed: u32;
  biggest: u32;
};
let job: PlaceJob | null = null;
/** A placement has been asked for since the last one began. */
let wanted = false;
let wanted_at: f64 = 0;
let last_start: f64 = -Infinity;

/** Asks for a placement. Called on every input the placement depends on —
 * the view, the centre, a page, the setting, a resize, terrain arriving,
 * the index finishing — and cheap enough to be: it sets a flag the content
 * step reads, throttled there to one run per PLACEMENT_MS while inputs
 * keep changing, and once more after they stop. */
export function poi_place_request(): void {
  if (!wanted) wanted_at = performance.now();
  wanted = true;
}

/** True while a placement is pending or running — the tick must keep
 * coming, since nothing else may be asking. */
function poi_place_busy(): boolean {
  return wanted || job !== null;
}

function* units(): IterableIterator<number> {
  while (job) yield job.stage;
}

/**
 * Advances the placement under the budget. Returns true on the step that
 * commits one.
 */
function poi_place_step(
  budget: FrameBudget,
  now: f64,
  map_center: PVF3 | null,
  show_poi: boolean,
  viewport_w: f64,
  viewport_h: f64,
): boolean {
  if (!job) {
    if (!wanted || now - last_start < PLACEMENT_MS) return false;
    wanted = false;
    last_start = now;
    if (!map_center || !show_poi) {
      if (CORE_PXL) CORE_PXL.pxl_billboards_write(badge_set(CORE_PXL), 0, 0);
      invalidate();
      return true;
    }
    job = {
      stage: QUERY,
      t0: now,
      wanted_at,
      frames: 0,
      rebuilds: 0,
      map_origin: get_local_world_origin(map_center),
      cx: 0,
      cy: 0,
      cz: 0,
      sync_ms: 0,
      hits: 0,
      h: 0,
      n: 0,
      kept: 0,
      placed: 0,
      k: 0,
      picks: [],
      absorbed: 0,
      biggest: 0,
    };
  }
  const j = job;
  j.frames++;
  // The live index moved under the hits. Everything filed so far named
  // entries of the old lanes; back to the query.
  if (j.stage > QUERY && poi_index_stats().rebuilds !== j.rebuilds) j.stage = QUERY;

  let committed = false;
  budget.run(units(), (stage) => {
    switch (stage) {
      case QUERY:
        query_stage(j);
        return true;
      case FILE:
        file_stage(j);
        return true;
      case OFFER:
        offer_stage(j);
        return true;
      case SELECT:
        select_stage(j, viewport_w, viewport_h);
        return true;
      case PLACE:
        place_stage(j, viewport_w, viewport_h);
        return true;
      case BUILD:
        build_stage(j);
        return true;
      default:
        commit_stage(j, now);
        committed = true;
        job = null;
        return true;
    }
  });
  return committed;
}

function query_stage(j: PlaceJob): void {
  const t0 = performance.now();
  // Everything is measured from the camera, as the chunks are (their tier
  // is their distance from the camera, sched/chunk_table.ts): the disc the
  // index holds, the reach a candidate is inside, the tier it is filed at.
  // Measured from the pivot instead, a wide zoom followed by a tilt down to
  // the ground filed the places under the camera at the tier of the pivot's
  // distance, snapped to cells of 2^n across, while the fade — which is by
  // camera distance (billboard/distance_fade.ts) — drew them as if near.
  // In the frame's own coordinates, where the index's lanes are.
  j.cx = view.eye_local.x;
  j.cy = view.eye_local.y;
  j.cz = view.eye_local.z;
  $cam_v3[0] = j.cx;
  $cam_v3[1] = j.cy;
  $cam_v3[2] = j.cz;
  const centre: PVF3 = local_world_to_canonical($centre, $cam_v3, j.map_origin);

  // Only asks; the rebuild itself is the poi_index step's, a range of
  // records per unit, and the live index keeps answering meanwhile.
  sync_poi_index(centre, j.map_origin, local_pos_from_absolute, poi_flat_pos);
  j.sync_ms = performance.now() - t0;
  j.rebuilds = poi_index_stats().rebuilds;
  j.hits = query_poi_index(j.cx, j.cy, j.cz, fade_end_u(), $hits);
  j.h = 0;
  $nodes.begin();
  $batch.reset();
  j.stage = FILE;
}

/** Files the next FILE_UNIT hits under the lattice node each belongs to at
 * its own tier. Nothing here is a POI — a node is an address and a count. */
function file_stage(j: PlaceJob): void {
  const far_end = fade_end_u();
  const far_end2 = far_end * far_end;
  const end = Math.min(j.hits, j.h + FILE_UNIT);
  for (; j.h < end; j.h++) {
    const i = $hits[j.h];

    // The exact reach, isotropic, on the frame-local lanes the index already
    // holds — plain float arithmetic with no PVF3 and no BigInt in the loop.
    // Both sides carry a real z: entries are grounded at build time
    // (poi_index.ts) and the camera has its own height, so a POI in a valley
    // below it is correctly further away than one at eye level the same
    // distance across.
    const dx = poi_x(i) - j.cx, dy = poi_y(i) - j.cy, dz = poi_z(i) - j.cz;
    const d2 = dx * dx + dy * dy + dz * dz;
    if (d2 > far_end2) continue;

    // Indexed before its tile arrived, so its z is the bake's flat zero and
    // drawing it would put the badge underground. It stays in the index —
    // a rebuild on tile arrival is what grounds it.
    if (!poi_grounded(i)) continue;

    // Its own tier, from its own distance. Not one tier for the frame: a
    // perspective view holds near and far at once, and a single tier is
    // wrong at whichever end it was not chosen for.
    $nodes.add(poi_cell_x(i), poi_cell_y(i), poi_cell_z(i), tier_shamt(Math.sqrt(d2), i), i, d2);
  }
  if (j.h >= j.hits) {
    j.n = 0;
    j.stage = OFFER;
  }
}

/** Offers the next OFFER_UNIT nodes to the screen, each measured — the
 * cold measurement is a canvas measureText per name, which is why this is
 * a stage of its own rather than part of filing. */
function offer_stage(j: PlaceJob): void {
  const end = Math.min($nodes.count, j.n + OFFER_UNIT);
  for (; j.n < end; j.n++) {
    const n = j.n;
    const shamt = $nodes.shamt[n];
    const members = $nodes.members[n];
    const rep = $nodes.first[n];

    // The badge stands at the centre of the cell, on all three axes, with
    // the half following the tier. Integers plus a power-of-two half, so it
    // is exact — and every member of the cell resolves to it, which is what
    // stops the badge moving when the camera does.
    const nx = $nodes.centre($nodes.key_x[n], shamt);
    const ny = $nodes.centre($nodes.key_y[n], shamt);
    const nz = $nodes.centre($nodes.key_z[n], shamt);

    const logical_w = members > 1 ? cluster_badge_logical_w(members) : poi_badge_logical_w(poi_page(rep), poi_rec(rep));
    $batch.push(n, nx, ny, nz, logical_w, BADGE_COLLIDE_H, BADGE_CENTER_Y, poi_tiebreak(rep));
  }
  if (j.n >= $nodes.count) j.stage = SELECT;
}

/** The frustum pass and the sort, one unit; then placement a range at a
 * time (PLACE). */
function select_stage(j: PlaceJob, viewport_w: f64, viewport_h: f64): void {
  j.k = 0;
  j.picks.length = 0;
  j.absorbed = 0;
  j.biggest = 0;
  select_begin($batch, viewport_w, viewport_h);
  j.kept = 0;
  j.placed = 0;
  j.stage = PLACE;
}

function place_stage(j: PlaceJob, viewport_w: f64, viewport_h: f64): void {
  j.placed += PLACE_UNIT;
  j.kept = select_place($batch, MAX_RENDERED, viewport_w, viewport_h, $kept, j.placed);
  if (select_remaining() === 0) j.stage = BUILD;
}

/** Resolves the next BUILD_UNIT kept nodes into picks and draws the badges
 * that are not yet on the sheet — the canvas work, a few per unit. */
function build_stage(j: PlaceJob): void {
  const end = Math.min(j.kept, j.k + BUILD_UNIT);
  for (; j.k < end; j.k++) {
    const i = $kept[j.k];
    // Two counts, added rather than either replacing the other: what the
    // lattice already gathered, and what the screen gathered on top of it.
    // Only the last stage has to be lossless, so an earlier one may change
    // how something is represented but never how many it stands for.
    const pick = pick_of($batch.src[i], $batch.absorbed[i], j.map_origin);
    j.absorbed += pick.members - 1;
    if (pick.members > j.biggest) j.biggest = pick.members;
    // Drawn now, into the sheet, so the commit only points at rectangles.
    const cluster = pick.members > 1;
    pick.slot = cluster ? cluster_badge_slot(pick.members) : poi_badge_slot(pick.page, pick.rec);
    pick.logical_w = cluster ? cluster_badge_logical_w(pick.members) : poi_badge_logical_w(pick.page, pick.rec);
    j.picks.push(pick);
  }
  if (j.k >= j.kept) j.stage = COMMIT;
}

/** Floats a badge takes in the core's block (crates/pxl-core/src/world/billboard.rs). */
const BADGE_FLOATS = 10;
let $badge_set: u32 | null = null;
/** The core's billboard set the badges are, made on first use. */
function badge_set(pxl: NonNullable<typeof CORE_PXL>): u32 {
  if ($badge_set === null) $badge_set = pxl.pxl_billboards_new(BILLBOARD_ORDER.poi);
  return $badge_set;
}

/** Writes the whole placement in one step into the core, which holds the
 * badges against the placement's origin until the next commit. */
function commit_stage(j: PlaceJob, now: f64): void {
  const lane = FIXED_ALLOC.take(Float32Array, Math.max(1, j.picks.length) * BADGE_FLOATS);
  let count: u32 = 0;
  for (const pick of j.picks) {
    // NO_SLOT when the atlas is full. Nothing is drawn for it this pass,
    // which is the honest answer — the alternative is a rectangle
    // belonging to some other badge.
    if (pick.slot === NO_SLOT) continue;
    const s = pick.slot, o = count++ * BADGE_FLOATS;
    lane[o] = pick.local_x;
    lane[o + 1] = pick.local_y;
    lane[o + 2] = pick.local_z;
    lane[o + 3] = pick.logical_w;
    lane[o + 4] = BADGE_LOGICAL_H;
    lane[o + 5] = badge_atlas.u0(s);
    lane[o + 6] = badge_atlas.v0(s);
    lane[o + 7] = badge_atlas.u1(s);
    lane[o + 8] = badge_atlas.v1(s);
    lane[o + 9] = BADGE_LOGICAL_H > 0 ? BADGE_CENTER_Y / BADGE_LOGICAL_H : 1;
  }
  if (CORE_PXL) {
    const set = badge_set(CORE_PXL), origin = j.map_origin;
    CORE_PXL.pxl_billboards_place(set, ivec.to_real(ivec.X(origin)), ivec.to_real(ivec.Y(origin)), ivec.to_real(ivec.Z(origin)));
    CORE_PXL.pxl_billboards_write(set, count, lane.byteOffset);
  }
  FIXED_ALLOC.give(lane);
  // What changed has to ask for the frame that shows it.
  invalidate();

  poi_place_stats.absorbed = j.absorbed;
  poi_place_stats.biggest = j.biggest;
  poi_place_stats.ms = now - j.t0;
  poi_place_stats.sync_ms = j.sync_ms;
  if (j.sync_ms > poi_place_stats.max_sync_ms) poi_place_stats.max_sync_ms = j.sync_ms;
  // A worst case, not a percentile. Placement runs a handful of times a
  // second, so a window would hold too few samples to say anything a max
  // does not — and the question this answers is whether one run can ever
  // cost a frame, which is a max question. `ms` spans frames now, so the
  // per-frame cost is the step table's; this is the latency.
  if (poi_place_stats.ms > poi_place_stats.max_ms) poi_place_stats.max_ms = poi_place_stats.ms;
  if (j.hits > poi_place_stats.max_hits) poi_place_stats.max_hits = j.hits;
  poi_place_stats.wait_ms = now - j.wanted_at;
  if (poi_place_stats.wait_ms > poi_place_stats.max_wait_ms) poi_place_stats.max_wait_ms = poi_place_stats.wait_ms;
  poi_place_stats.frames = j.frames;
  if (j.frames > poi_place_stats.max_frames) poi_place_stats.max_frames = j.frames;
  poi_place_stats.runs++;
  poi_place_stats.hits = j.hits;
  poi_place_stats.candidates = $batch.count;
  poi_place_stats.kept = j.kept;
}

const $node_addr: PVF3 = { i: 0n, f: 0n };

/**
 * One node turned into the record React mounts — the only object this file
 * builds, and it builds as many as are drawn rather than one per candidate.
 *
 * The anchor is resolved here and nowhere else: a lattice address plus the
 * half its tier is worth. One BigInt per drawn badge, against the thousands
 * per tick the old chain converted.
 */
function pick_of(node: u32, absorbed_extra: u32, map_origin: IVec): PoiPick {
  const shamt = $nodes.shamt[node];
  const base: IVec = ivec.add(
    map_origin,
    ivec.gather(BigInt($nodes.key_x[node] << shamt), BigInt($nodes.key_y[node] << shamt), BigInt($nodes.key_z[node] << shamt)),
  );
  const anchor = SPAN_CENTER($node_addr, base, shamt);
  const [lx, ly, lz] = local_pos_from_absolute(anchor, map_origin);
  return {
    local_x: lx,
    local_y: ly,
    local_z: lz,
    // From the absolute address, never from the node's own coordinates.
    // Those are relative to the frame origin, which jumps 4096u on an L4
    // crossing — so the same physical cell would take a new identity every
    // time the origin moved, and every badge on screen would remount at
    // once. The anchor does not move, only the name for it would have.
    key: `${shamt}:${anchor.i}`,
    page: poi_page($nodes.first[node]),
    rec: poi_rec($nodes.first[node]),
    anchor_i: anchor.i,
    anchor_f: anchor.f,
    members: $nodes.members[node] + absorbed_extra,
    slot: NO_SLOT,
    logical_w: 0,
  };
}

/**
 * How long placement is allowed to lag the camera.
 *
 * Placement does not have to run every frame — Mapbox GL's own conclusion,
 * and the implementation this file replaced reached it independently: it
 * promoted the nearest few to the expensive treatment every 0.2s (see this
 * file's own header). What the interval buys here is the whole per-candidate
 * pass, which in the densest part of the bake is thousands of terrain
 * lookups; at 60fps this is an 12x reduction in how often that runs.
 *
 * A badge is a text label rather than a cursor, so it arriving a frame or
 * two after the camera settles reads as nothing at all. Note also that the
 * anchors themselves never lag: a mounted badge is positioned by AnchorNode
 * every frame regardless. What waits is only the decision about which set is
 * mounted.
 */
const PLACEMENT_MS = 200;

export function PoiLayer() {
  const poi_revision = usePoiStore((state) => state.revision);
  const map_center = useMapStore((state) => state.map_center);
  const show_poi = useSettingsStore((state) => state.show_poi);
  // view_state: a pure rotate/zoom changes what's in frustum without
  // changing map_center.
  const view_state = useCameraStore((state) => state.view_state);

  // Every input a placement depends on asks for one; the step throttles.
  // The view and the centre change every frame of a drag, and asking is a
  // flag. Everything else — a page arriving, the layer switched on, a
  // resize — is a change no amount of waiting improves, and asks the same
  // way; the step runs once PLACEMENT_MS have passed since the last began.
  useEffect(() => {
    poi_place_request();
    invalidate();
  }, [view_state, map_center, poi_revision, show_poi]);

  // One subscription for the whole layer, not one per mounted badge. Terrain
  // arriving is what grounds an entry indexed before its tile did, and the
  // index is where grounding happens now — so the response is to rebuild it
  // and re-place, rather than to have each badge re-probe for itself.
  useEffect(
    () =>
      register_tile_interest(() => true, () => {
        invalidate_poi_index();
        poi_place_request();
        invalidate();
      }),
    [],
  );

  // The index is rebuilt a range of records per step in the frame's content
  // phase, and while that runs the previous build keeps answering — so a
  // placement can land against an index a few pages behind. The step that
  // finishes a rebuild is the moment to place again, and the tick must keep
  // coming until it does: nothing else may be asking.
  useTick("content", ({ state, budget }) => {
    if (poi_index_sweep(budget)) poi_place_request();
    if (poi_index_stats().sweeping > 0) state.invalidate();
  }, "poi_index");

  // The placement itself, after the index's step so a finished rebuild is
  // placed against in the same frame.
  useTick("content", ({ state, budget }) => {
    poi_place_step(budget, performance.now(), map_center, show_poi, view.width, view.height);
    if (poi_place_busy()) state.invalidate();
  }, "poi_place");

  return null;
}
