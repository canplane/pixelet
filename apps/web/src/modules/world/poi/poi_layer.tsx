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

import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { useThree } from "@react-three/fiber";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { FROM_L3_IDX, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import type { POI } from "@pxl/shared/bake/model/feature.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { get_local_world_origin, local_world_to_canonical } from "../frame/coord.ts";
import { useFrameRefs } from "../frame/context.tsx";
import { with_ele } from "../terrain/elevation.ts";
import { register_tile_interest } from "../terrain/store.ts";
import { CHEBYSHEV_DIST } from "../terrain/use_tile_fetcher.ts";
import { AnchorNode } from "../anchor/node.tsx";
import { FAR_END } from "../billboard/distance_fade.ts";
import { FONT_FAMILY, FONT_WEIGHT_REGULAR } from "../billboard/canvas_scale.ts";
import { Billboard, BILLBOARD_ORDER, type BuiltBillboard } from "../billboard/billboard.tsx";
import { type BillboardCandidate, select_visible_billboards } from "../billboard/frustum_budget.ts";
import { resolve_poi_theme } from "./poi_theme.ts";
import { usePoiStore } from "./store.ts";

// Global cap across every loaded page, not per-page — see this file's own
// top doc comment on why this isn't the old implementation's own two-tier
// LOD. Picked well above the old MAX_HTML_POIS (20) since a single flat
// badge here is far cheaper than that version's DOM+occlusion-raycast Html
// element was.
const MAX_RENDERED = 80;

const FONT = `${FONT_WEIGHT_REGULAR} 12px ${FONT_FAMILY}`;
const ICON_FONT = "14px sans-serif";
const NAME_MAX_CHARS = 12;
const ICON_SIZE = 20;
// Icon-to-text gap and text's own right-edge margin — kept equal
// deliberately (the actual ask: not "pad both ends of the pill the same,"
// the icon end has no padding of its own at all, it sits flush against the
// pill's own rounded left cap — just "the text's own left and right
// breathing room should match each other").
const GAP = 8;
const PAD_Y = 7;
// Same supersampling reasoning as chat_bubble.tsx's own RENDER_SCALE — a
// canvas sized directly off FONT's own px metrics blurs once magnified onto
// a sprite occupying many more screen pixels close to the camera.
const RENDER_SCALE = 3;

// checkpoint/20260107-float's own "Anchor Dot" (poi-item.tsx) — a small dot
// under the pill marking the actual ground point, since the pill itself
// floats above and offset from it.
const DOT_DIAM = 6;
const DOT_GAP = 4;
const DOT_BORDER = 1;

/**
 * Where the anchor dot sits, above the elevation with_ele grounds to.
 *
 * Exactly half a cell, and nothing else added on top. `ele` is the top
 * *face* of the topmost terrain cell — the mesh fills up to `ele - 1`
 * (terrain/adapter.ts) — so half a cell above it is the centre of the cell
 * the dot marks. Terrain cells are centred and a POI is a point on one.
 *
 * This is also the point billboard/anchor_occlusion.ts measures from, which
 * is why it is one number rather than a ground height plus a separate air
 * gap: the place the dot claims and the place visibility is judged from
 * have to be the same place, or the tolerance is being applied to a
 * position nothing else agrees with.
 *
 * X and Y are untouched by any of this. A point marker has to land exactly
 * where the place is; only Z is aligned to terrain, and terrain is cells.
 */
const ANCHOR_Z_U: f64 = 0.5;

/** A single-line pill (colored icon circle + name) with an anchor dot
 * centered below it — the shape `checkpoint/20260107-float`'s own HTML
 * badge used (`poi-item.tsx`), redrawn as one flat canvas texture instead
 * of a DOM element (see this file's own top doc comment on why hover/
 * hover-lift/day-night from that version aren't reproduced here). Truncates
 * by character count rather than wrap_text's own word-wrap — a map badge
 * reads as one short line, not a multi-line chat message. */
function build_poi_badge_canvas(poi: POI): BuiltBillboard {
  const theme = resolve_poi_theme(poi.category);
  const name = poi.name.length > NAME_MAX_CHARS ? `${poi.name.slice(0, NAME_MAX_CHARS - 1)}…` : poi.name;

  const measure_ctx = document.createElement("canvas").getContext("2d")!;
  measure_ctx.font = FONT;
  const text_w = Math.ceil(measure_ctx.measureText(name).width);

  const pill_h = ICON_SIZE + PAD_Y;
  const r = pill_h / 2;
  const text_x = r + ICON_SIZE / 2 + GAP;
  const logical_w = text_x + text_w + GAP;
  const logical_h = pill_h + DOT_GAP + DOT_DIAM;

  const canvas = document.createElement("canvas");
  canvas.width = logical_w * RENDER_SCALE;
  canvas.height = logical_h * RENDER_SCALE;
  const ctx = canvas.getContext("2d")!;
  ctx.imageSmoothingEnabled = false;
  ctx.scale(RENDER_SCALE, RENDER_SCALE);

  // Pill background — a rect with fully-rounded ends (radius = half height).
  ctx.fillStyle = "rgba(20,20,26,0.72)";
  ctx.beginPath();
  ctx.roundRect(0, 0, logical_w, pill_h, r);
  ctx.fill();

  // Icon circle is concentric with the pill's own rounded left cap (same
  // center `r`, smaller radius) rather than padded off it — no separate
  // left-margin constant, `GAP` is the one deliberate breathing-room value
  // in this layout: icon-to-text and text-to-right-tip, equal on purpose.
  const icon_cx = r;
  const icon_cy = pill_h / 2;
  ctx.fillStyle = theme.color;
  ctx.beginPath();
  ctx.arc(icon_cx, icon_cy, ICON_SIZE / 2, 0, Math.PI * 2);
  ctx.fill();
  // Tried pixel-scanning each glyph's actual rendered ink to correct for
  // per-emoji font-metric quirks (2026-09-08) — still came out wrong for at
  // least one glyph, and font-substitution behavior for color emoji isn't
  // worth chasing further than this: plain center/middle, one hand nudge.
  ctx.font = ICON_FONT;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(theme.icon, icon_cx, icon_cy + 1);

  // Name, left-aligned right after the icon circle.
  ctx.font = FONT;
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  ctx.fillStyle = "#fff";
  ctx.fillText(name, text_x, pill_h / 2 + 1);

  // Anchor dot, centered under the whole pill.
  const dot_cx = logical_w / 2;
  const dot_cy = pill_h + DOT_GAP + DOT_DIAM / 2;
  ctx.fillStyle = "rgba(255,255,255,0.6)";
  ctx.beginPath();
  ctx.arc(dot_cx, dot_cy, DOT_DIAM / 2, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = "rgba(255,255,255,0.25)";
  ctx.lineWidth = DOT_BORDER;
  ctx.stroke();

  return { canvas, logical_w, logical_h, center_y: dot_cy };
}

/** center.xy (page-local real units, overture_provider.ts)
 * resolved against the page's own global corner — no elevation grounding,
 * unlike poi_anchor below. Used only for the distance ranking in
 * select_nearby_pois: cheap (no per-candidate terrain/tile lookup) since it
 * runs over every loaded POI, not just the ones that end up rendered. */
function poi_flat_pos(poi: POI, page_idx: IVec): PVF3 {
  return local_world_to_canonical({ i: 0n, f: 0n }, poi.center, FROM_L3_IDX(page_idx));
}

/** Squared XY distance between two absolute PVF3 positions, in real [u]
 * units — a ranking distance, not a precise one: `b`'s own fractional (`f`)
 * offset is ignored (only `a.f` is kept), which is at most ~1u of error,
 * irrelevant for sorting hundreds of candidates that are typically tens to
 * hundreds of u apart. */
function flat_dist2(a: PVF3, b: PVF3): f64 {
  const local: PVF3 = { i: ivec.sub(a.i, b.i), f: a.f };
  const dx = pvf3.to_real(pvf3.X(local));
  const dy = pvf3.to_real(pvf3.Y(local));
  return dx * dx + dy * dy;
}

/** Up to MAX_RENDERED POIs, deduped by id — a real feature near a page
 * boundary can come back from more than one page's own bbox query
 * (adjacent pages' queries legitimately overlap at the edges), and without
 * this it would render twice, once per page, at the exact same position
 * (`checkpoint/20260107-float`'s own PoiLayer hit the identical case and
 * solved it the same way — dedupe by id, keep whichever copy is seen
 * first). Distance (FAR_END below) only rules out what's too far to ever
 * render — final selection is select_visible_billboards' own job (real
 * camera-frustum membership, then a stable per-id cut if still over
 * budget), not poi_extractor.ts's own rank field: rank is "how notable is
 * this in general", not "is this visible right now". */
const FAR_END_U2 = FAR_END * FAR_END;

// FAR_END=320 is under L3_RANGE=1024, so a page beyond Chebyshev distance 1
// from the camera's own page can never contain anything within FAR_END.
const NEARBY_PAGE_RADIUS: u32 = 1;

/** absolute, projected into local_world_frame's own origin (map_origin) —
 * the frame select_visible_billboards' local_x/y/z expects (see
 * AnchorNode's own local_pos in frame/coord.ts). */
function local_pos_from_absolute(absolute: PVF3, map_origin: IVec): [f64, f64, f64] {
  const local: PVF3 = { i: ivec.sub(absolute.i, map_origin), f: absolute.f };
  return [pvf3.to_real(pvf3.X(local)), pvf3.to_real(pvf3.Y(local)), pvf3.to_real(pvf3.Z(local))];
}

function select_nearby_pois(
  poi_map: ReadonlyMap<IVec, readonly POI[]>,
  map_center: PVF3,
  local_world_frame: THREE.Object3D | null,
  camera: THREE.Camera,
): { poi: POI; page_idx: IVec }[] {
  // L3_IDX shifts X/Y/Z together; TO_XY drops Z (matches store.ts's own
  // page keys). Without it, map_center.i.Z (real elevation) can push
  // CHEBYSHEV_DIST past NEARBY_PAGE_RADIUS regardless of X/Y distance.
  const center_page_idx: IVec = TO_XY(L3_IDX(map_center.i));
  const seen = new Set<string>();
  const candidates: { poi: POI; page_idx: IVec; absolute: PVF3; dist2: f64 }[] = [];

  for (const [page_idx, pois] of poi_map) {
    if (CHEBYSHEV_DIST(ivec.sub(page_idx, center_page_idx)) > NEARBY_PAGE_RADIUS) continue;
    for (const poi of pois) {
      if (seen.has(poi.id)) continue;
      seen.add(poi.id);
      const absolute = poi_flat_pos(poi, page_idx);
      // Flat distance from map_center, and only as a coarse reach limit —
      // never as the ranking. It is the wrong measure for ranking twice
      // over: it is measured from the look-at target rather than from the
      // eye, and it is a projection onto the ground rather than a distance
      // in space. At an oblique polar the camera sits well off to the
      // target's -Y side (camera/orbit.ts), so the POIs nearest the *eye*
      // are the ones furthest from the *target* on that side, and ranking
      // this way drops exactly the badges filling the bottom of the screen
      // while keeping ones near the horizon. Looking straight down the two
      // measures agree, which is why the artifact vanishes there.
      if (flat_dist2(absolute, map_center) > FAR_END_U2) continue;
      candidates.push({ poi, page_idx, absolute, dist2: 0 });
    }
  }

  const map_origin = get_local_world_origin(map_center);
  // Grounded, not the flat z=0 poi_flat_pos gives: the frustum test and the
  // ranking both have to use the position the badge is actually at. A POI on
  // a hill tested at z=0 sits far below where it is drawn, and at an oblique
  // angle that vertical error moves it across the screen — which is the same
  // reason this only ever misbehaved away from straight-down.
  for (const c of candidates) {
    const grounded = with_ele({ i: 0n, f: 0n }, c.absolute.i, c.absolute.f);
    if (grounded) c.absolute = grounded;
  }

  // local_world_frame_ref.current starts null until mount. Falls back to the
  // flat measure, which is all that is available without a frame to resolve
  // world positions against.
  if (!local_world_frame) {
    for (const c of candidates) c.dist2 = flat_dist2(c.absolute, map_center);
    candidates.sort((a, b) => a.dist2 - b.dist2);
    return candidates.slice(0, MAX_RENDERED);
  }

  const billboard_candidates: BillboardCandidate<{ poi: POI; page_idx: IVec }>[] = candidates.map((c) => {
    const [x, y, z] = local_pos_from_absolute(c.absolute, map_origin);
    return { item: { poi: c.poi, page_idx: c.page_idx }, id: c.poi.id, local_x: x, local_y: y, local_z: z };
  });
  return select_visible_billboards(billboard_candidates, local_world_frame, camera, MAX_RENDERED);
}

/** One POI's own absolute anchor — poi_flat_pos, then grounded to the real
 * terrain surface: center.z is always 0 coming from the bake
 * (overture_provider.ts zeroes it deliberately, since a POI's real height
 * was never the bake's concern), so without this a badge would sit at the
 * page's own Z origin instead of floating above wherever the ground
 * actually is. */
function poi_anchor(poi: POI, page_idx: IVec): PVF3 | null {
  const flat = poi_flat_pos(poi, page_idx);
  return with_ele(flat, flat.i, flat.f);
}

function PoiBadge({ poi, page_idx }: { poi: POI; page_idx: IVec }) {
  // poi_anchor can come back null on first evaluation if this page's terrain
  // hasn't loaded yet — a real race against use_poi_fetcher.ts's own
  // independent fetch, most likely to land badly on a cold cache. Without
  // this, nothing ever re-renders this badge once terrain does arrive (this
  // component keeps its identity across PoiLayer's own re-selections as long
  // as it stays in the top MAX_RENDERED, so a plain useMemo would stay stuck
  // at its first, terrain-less null forever). Same fix markers/location_puck.tsx
  // already applies for the same class of race.
  const [ground_tick, set_ground_tick] = useState(0);
  useEffect(() => register_tile_interest((p) => p === page_idx, () => set_ground_tick((t) => t + 1)), [page_idx]);

  const anchor = useMemo(() => poi_anchor(poi, page_idx), [poi, page_idx, ground_tick]);
  const built = useMemo(() => build_poi_badge_canvas(poi), [poi]);

  if (!anchor) return null;

  return (
    <AnchorNode i={anchor.i} f={anchor.f}>
      <Billboard built={built} base_z={ANCHOR_Z_U} render_order={BILLBOARD_ORDER.poi} />
    </AnchorNode>
  );
}

export function PoiLayer() {
  const poi_map = usePoiStore((state) => state.poi_map);
  const map_center = useMapStore((state) => state.map_center);
  const show_poi = useSettingsStore((state) => state.show_poi);
  // view_state: a pure rotate/zoom changes what's in frustum without
  // changing map_center.
  const view_state = useCameraStore((state) => state.view_state);
  const { local_world_frame_ref } = useFrameRefs();
  const { camera } = useThree();

  const selected = useMemo(() => {
    if (!map_center || !show_poi) return [];
    return select_nearby_pois(poi_map, map_center, local_world_frame_ref.current, camera);
    // local_world_frame_ref/camera: stable identities, not reactive state.
  }, [poi_map, map_center, show_poi, view_state]);

  if (!show_poi) return null;

  return (
    <>
      {selected.map(({ poi, page_idx }) => <PoiBadge key={poi.id} poi={poi} page_idx={page_idx} />)}
    </>
  );
}
