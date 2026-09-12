// @pxl/web/modules/world/poi/poi_layer.tsx

/**
 * POI's own render — world-anchored-billboard tier (see
 * discussions/20260907-world-rendering-tiers.md), billboard/billboard.tsx's
 * second consumer after chat_bubble.tsx. A category-themed badge
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

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { invalidate, useThree } from "@react-three/fiber";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { FROM_L3_IDX, SPAN_CENTER } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { POI } from "@pxl/shared/bake/model/feature.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { get_local_world_origin, local_world_to_canonical } from "../frame/coord.ts";
import { useFrameRefs } from "../frame/context.tsx";
import { register_tile_interest } from "../terrain/store.ts";
import { FAR_END } from "../billboard/distance_fade.ts";
import { BILLBOARD_ORDER } from "../billboard/billboard.tsx";
import { select_visible_billboards } from "../billboard/frustum_budget.ts";
import { BillboardBatch } from "../billboard/batch.ts";
import { CellNodes } from "../billboard/cell_nodes.ts";
import { BillboardField } from "../billboard/field.ts";
import {
  invalidate_poi_index,
  poi_at,
  poi_cell_x,
  poi_cell_y,
  poi_cell_z,
  poi_grounded,
  poi_index_stats,
  poi_index_sweep,
  poi_tiebreak,
  poi_x,
  poi_y,
  poi_z,
  query_poi_index,
  sync_poi_index,
} from "./poi_index.ts";
import { badge_atlas, cluster_badge, cluster_badge_metrics, poi_badge, poi_badge_metrics } from "./poi_badge.ts";
import { usePoiStore } from "./store.ts";
import { useTick } from "@/modules/world/sched/tick.ts";

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

/**
 * How wide a badge is, in CSS pixels, for picking the tier below.
 *
 * The pill is about this across at full strength (poi_badge.ts), and a cell
 * about a badge wide is the size at which "one badge per cell" and "badges
 * do not sit on each other" are the same statement.
 */
const BADGE_PX: f64 = 100;

/** The finest and coarsest cell a badge is allowed to snap to, as shifts of
 * the canonical unit. 0 is 1u, the terrain cell itself; 6 is 64u, which is
 * an L2 chunk and the top of the ladder terrain and decor already climb
 * (render/lod.ts's 1/2/4/.../64). Beyond that the tier belongs to the page,
 * not to the badge. */
const MIN_SHAMT: u32 = 0;
const MAX_SHAMT: u32 = 6;

/**
 * Which lattice tier badges snap to at this zoom.
 *
 * `px_per_u` is what use_distance_fade computes to hold a billboard at a
 * constant screen size, so this asks how much world a badge covers right
 * now and takes the cell nearest to it. Zooming in walks the tier down and
 * the badges spread; zooming out walks it up and they gather. It is the same
 * ladder terrain climbs, by the same argument, which is why it needs no
 * quantisation of its own.
 */
function tier_shamt(px_k: f64, dist: f64): u32 {
  // px_k is the projection with the distance divided out — CSS pixels per
  // world unit *times* distance — so one measurement serves every candidate
  // and each pays a divide rather than a trig call.
  const want_u = BADGE_PX * Math.max(1e-3, dist) / px_k;
  const shamt = Math.round(Math.log2(Math.max(1, want_u)));
  return Math.max(MIN_SHAMT, Math.min(MAX_SHAMT, shamt)) as u32;
}

/** center.xyz (page-local real units, overture_provider.ts) resolved against
 * the page's own global corner, untouched. The index grounds it and then
 * snaps it to a cell centre, so nothing here adds a half of its own — that
 * used to be a lone `+ 0.5` on Z, which is the right number at exactly one
 * tier and on exactly one axis.
 *
 * Handed to the index as a callback rather than imported there, so the index
 * does not have to know how a POI is addressed. */
function poi_flat_pos(out: PVF3, poi: POI, page_idx: IVec): PVF3 {
  return local_world_to_canonical(out, poi.center, FROM_L3_IDX(page_idx));
}

/** The reach, squared. It only rules out what is too far to ever draw —
 * which of the survivors actually draws is select_visible_billboards' job
 * (frustum membership, then nearest-first greedy placement against the
 * screen), and never poi_extractor.ts's rank field: rank is "how notable is
 * this in general", not "is this visible right now". */
const FAR_END_U2 = FAR_END * FAR_END;

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
const $hits = new Uint32Array(16384);
const $batch = new BillboardBatch();
const $nodes = new CellNodes(4096);
const $field = new BillboardField(BILLBOARD_ORDER.poi);
const $kept = new Uint32Array(MAX_RENDERED);

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
export const poi_place_stats = { ms: 0, max_ms: 0, sync_ms: 0, max_sync_ms: 0, runs: 0, hits: 0, max_hits: 0, candidates: 0, kept: 0, absorbed: 0, biggest: 0 };

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
  poi: POI;
  anchor_i: IVec;
  anchor_f: bigint;
  /** How many places this badge stands for, itself included. Above one it is
   * drawn as a count rather than as any of them: nothing here is a
   * representative, and `poi` survives only to key the list. */
  members: u32;
};

function select_nearby_pois(
  poi_map: ReadonlyMap<IVec, readonly POI[]>,
  map_center: PVF3,
  local_world_frame: THREE.Object3D | null,
  camera: THREE.Camera,
  viewport_w: f64,
  viewport_h: f64,
): PoiPick[] {
  const t0 = performance.now();
  const map_origin = get_local_world_origin(map_center);

  // Rebuilt when a page arrives or leaves, when the frame origin crosses an
  // L4 boundary, or when the camera leaves the disc the last build covers.
  // Everything below works on what a radius query returns rather than on
  // everything loaded — see poi_index.ts.
  // Pixels per world unit, times distance — the distance-free half of the
  // projection, so each candidate below resolves its own tier with a divide.
  const persp = camera as THREE.PerspectiveCamera;
  const px_k = viewport_h / (2 * Math.tan((persp.fov * Math.PI) / 360));
  sync_poi_index(poi_map, map_center, map_origin, local_pos_from_absolute, poi_flat_pos);
  const t_sync = performance.now();
  const [cx, cy, cz] = local_pos_from_absolute(map_center, map_origin);
  const hits = query_poi_index(cx, cy, cz, FAR_END, $hits);

  $batch.reset();
  // Two passes, and they are different questions. The first files every
  // candidate under the lattice node it belongs to at its own tier; the
  // second offers those nodes, not the candidates, to the screen. Nothing
  // between them is a POI — a node is an address and a count.
  $nodes.begin();
  for (let h = 0; h < hits; h++) {
    const i = $hits[h];

    // The exact reach, isotropic, on the frame-local lanes the index already
    // holds — plain float arithmetic with no PVF3 and no BigInt in the loop.
    // Both sides carry a real z: entries are grounded at build time
    // (poi_index.ts) and map_center sits on the terrain surface, so a POI in
    // a valley below the camera is correctly further away than one at eye
    // level the same distance across.
    const dx = poi_x(i) - cx, dy = poi_y(i) - cy, dz = poi_z(i) - cz;
    const d2 = dx * dx + dy * dy + dz * dz;
    if (d2 > FAR_END_U2) continue;

    // Indexed before its tile arrived, so its z is the bake's flat zero and
    // drawing it would put the badge underground. It stays in the index —
    // a rebuild on tile arrival is what grounds it.
    if (!poi_grounded(i)) continue;

    // Its own tier, from its own distance. Not one tier for the frame: a
    // perspective view holds near and far at once, and a single tier is
    // wrong at whichever end it was not chosen for.
    $nodes.add(poi_cell_x(i), poi_cell_y(i), poi_cell_z(i), tier_shamt(px_k, Math.sqrt(d2)), i, d2);
  }

  $batch.reset();
  for (let n = 0; n < $nodes.count; n++) {
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

    const m = members > 1 ? cluster_badge_metrics(members) : poi_badge_metrics(poi_at(rep));
    $batch.push(n, nx, ny, nz, m.logical_w, m.collide_h, m.center_y, poi_tiebreak(rep));
  }

  // local_world_frame_ref.current starts null until mount. Without a frame
  // there is no world position to resolve, so this falls back to the flat
  // measure and the near ones — which is all that is available.
  if (!local_world_frame) {
    const out: PoiPick[] = [];
    for (let i = 0; i < $batch.count && out.length < MAX_RENDERED; i++) out.push(pick_of($batch.src[i], 0, map_origin));
    record_place(t0, t_sync, hits, out.length);
    return out;
  }

  const kept = select_visible_billboards($batch, local_world_frame, camera, MAX_RENDERED, viewport_w, viewport_h, $kept);

  // The one place a record is built, and it is built for React rather than
  // for this pipeline: everything above is an index.
  const out: PoiPick[] = [];
  let absorbed = 0, biggest = 0;
  for (let k = 0; k < kept; k++) {
    const i = $kept[k];
    // Two counts, added rather than either replacing the other: what the
    // lattice already gathered, and what the screen gathered on top of it.
    // Only the last stage has to be lossless, so an earlier one may change
    // how something is represented but never how many it stands for.
    const pick = pick_of($batch.src[i], $batch.absorbed[i], map_origin);
    absorbed += pick.members - 1;
    if (pick.members > biggest) biggest = pick.members;
    out.push(pick);
  }
  poi_place_stats.absorbed = absorbed;
  poi_place_stats.biggest = biggest;
  record_place(t0, t_sync, hits, kept);
  return out;
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
    poi: poi_at($nodes.first[node]),
    anchor_i: anchor.i,
    anchor_f: anchor.f,
    members: $nodes.members[node] + absorbed_extra,
  };
}

function record_place(t0: f64, t_sync: f64, hits: u32, kept: u32): void {
  const ms = performance.now() - t0;
  const sync_ms = t_sync - t0;
  poi_place_stats.sync_ms = sync_ms;
  if (sync_ms > poi_place_stats.max_sync_ms) poi_place_stats.max_sync_ms = sync_ms;
  poi_place_stats.ms = ms;
  // A worst case, not a percentile. Placement runs a handful of times a
  // second, so a window would hold too few samples to say anything a max
  // does not — and the question this answers is whether one run can ever
  // cost a frame, which is a max question.
  if (ms > poi_place_stats.max_ms) poi_place_stats.max_ms = ms;
  if (hits > poi_place_stats.max_hits) poi_place_stats.max_hits = hits;
  poi_place_stats.runs++;
  poi_place_stats.hits = hits;
  poi_place_stats.candidates = $batch.count;
  poi_place_stats.kept = kept;
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

/**
 * A counter that advances at most every PLACEMENT_MS while its inputs keep
 * changing, and once more after they stop.
 *
 * Throttle rather than debounce, and the difference is the point: a debounce
 * re-arms on every camera tick, so a camera that never comes to rest would
 * never place at all. The trailing run is what makes the final state right
 * regardless of when the movement ended.
 */
function use_placement_epoch(...inputs: unknown[]): u32 {
  const [epoch, set_epoch] = useState<u32>(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const last_ms = useRef<f64>(0);

  useEffect(() => {
    if (timer.current !== null) return;
    const wait = Math.max(0, PLACEMENT_MS - (performance.now() - last_ms.current));
    timer.current = setTimeout(() => {
      timer.current = null;
      last_ms.current = performance.now();
      set_epoch((e) => e + 1);
    }, wait);
  }, inputs);

  // Clearing the handle is not enough — the ref has to be released too, or a
  // StrictMode remount finds a timer that is already cancelled, takes the
  // early return above, and never schedules another.
  useEffect(() => () => {
    if (timer.current === null) return;
    clearTimeout(timer.current);
    timer.current = null;
  }, []);

  return epoch;
}

export function PoiLayer() {
  const poi_map = usePoiStore((state) => state.poi_map);
  const map_center = useMapStore((state) => state.map_center);
  const show_poi = useSettingsStore((state) => state.show_poi);
  // view_state: a pure rotate/zoom changes what's in frustum without
  // changing map_center.
  const view_state = useCameraStore((state) => state.view_state);
  const { local_world_frame_ref } = useFrameRefs();
  const { camera, size } = useThree();

  const epoch = use_placement_epoch(view_state, map_center);

  // One subscription for the whole layer, not one per mounted badge. Terrain
  // arriving is what grounds an entry indexed before its tile did, and the
  // index is where grounding happens now — so the response is to rebuild it
  // and re-place, rather than to have each badge re-probe for itself.
  const [ground_tick, set_ground_tick] = useState<u32>(0);
  useEffect(
    () =>
      register_tile_interest(() => true, () => {
        invalidate_poi_index();
        set_ground_tick((t) => t + 1);
      }),
    [],
  );

  // The index is rebuilt a page per step in the frame's content phase, and
  // while that runs the previous build keeps answering — so a placement can
  // land against an index a few pages behind. The step that finishes a
  // rebuild is the moment to place again, and the tick must keep coming
  // until it does: under frameloop="demand" nothing else may be asking.
  const [index_tick, set_index_tick] = useState<u32>(0);
  useTick("content", ({ state, budget }) => {
    if (poi_index_sweep(budget)) set_index_tick((t) => t + 1);
    else if (poi_index_stats().sweeping > 0) state.invalidate();
  });

  // Deliberately not keyed on view_state or map_center: the epoch above is,
  // and it is what decides when the camera has moved enough to be worth
  // re-placing. Everything that is still keyed directly is a change no
  // amount of waiting improves — a page arriving, the layer being switched
  // on, a resize.
  const selected = useMemo(() => {
    if (!map_center || !show_poi) return [];
    return select_nearby_pois(poi_map, map_center, local_world_frame_ref.current, camera, size.width, size.height);
    // local_world_frame_ref/camera: stable identities, not reactive state.
  }, [poi_map, show_poi, size.width, size.height, epoch, ground_tick, index_tick]);

  // The badges themselves are written, not declared. `selected` changing is
  // a list of slots to fill, so this is where the imperative half is driven
  // from — and the only React work a placement causes is this effect, not a
  // subtree per badge.
  useEffect(() => {
    $field.begin();
    for (const pick of selected) {
      const built = pick.members > 1 ? cluster_badge(pick.members) : poi_badge(pick.poi);
      // Null when the atlas is full. Nothing is drawn for it this pass,
      // which is the honest answer — the alternative is a rectangle
      // belonging to some other badge.
      if (!built) continue;
      $field.push(pick.local_x, pick.local_y, pick.local_z, badge_atlas.texture, built.uv, built.logical_w, built.logical_h, built.center_y);
    }
    $field.end();
    // frameloop="demand": placement can finish on a tick where nothing else
    // asked for a frame — the trailing run after the camera has already come
    // to rest — so what it changes has to ask for one itself.
    invalidate();
  }, [selected]);

  // One loop for every badge, where there used to be one useFrame per badge.
  useTick("cosmetic", ({ state }) => $field.update(state.camera, state.size.height));

  // Attached to the frame the batch's coordinates are relative to, rather
  // than to an anchor of its own: every badge is within FAR_END of the
  // camera, so they all share the L4 cell the frame is aligned to, and a
  // per-badge AnchorNode was resolving the same anchor a few hundred times.
  useLayoutEffect(() => {
    const frame = local_world_frame_ref.current;
    if (!frame) return;
    frame.add($field.group);
    return () => {
      frame.remove($field.group);
    };
  }, [local_world_frame_ref.current]);

  return null;
}
