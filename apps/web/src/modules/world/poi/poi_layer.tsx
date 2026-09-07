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
 *   instead just picks the nearest MAX_RENDERED globally (see
 *   select_nearby_pois below) and renders exactly those. Fine at today's
 *   scale; revisit if it stops being fine.
 */

import { useMemo } from "react";

import type { f64 } from "@xpute/core/kit/word.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { POI } from "@pxl/shared/bake/model/osm_feature.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { local_world_to_canonical } from "../frame/coord.ts";
import { with_ele } from "../terrain/elevation.ts";
import { AnchorNode } from "../anchor/node.tsx";
import { FAR_END } from "../billboard/distance_fade.ts";
import { FONT_FAMILY } from "../billboard/canvas_scale.ts";
import { Billboard, type BuiltBillboard } from "../billboard/billboard.tsx";
import { resolve_poi_theme } from "./poi_theme.ts";
import { usePoiStore } from "./store.ts";

// Global cap across every loaded page, not per-page — see this file's own
// top doc comment on why this isn't the old implementation's own two-tier
// LOD. Picked well above the old MAX_HTML_POIS (20) since a single flat
// badge here is far cheaper than that version's DOM+occlusion-raycast Html
// element was.
const MAX_RENDERED = 80;

const FONT = `12px ${FONT_FAMILY}`;
const ICON_FONT = "14px sans-serif";
const NAME_MAX_CHARS = 12;
const ICON_SIZE = 20;
const ICON_GAP = 4;
const BADGE_PAD_X = 7;
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

// Air gap above the terrain surface the anchor dot's own center is grounded
// to (see build_poi_badge_canvas's own `center_y`).
const GROUND_CLEARANCE_U: f64 = 0.3;

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

  const pill_h = ICON_SIZE + BADGE_PAD_X;
  const logical_w = BADGE_PAD_X + ICON_SIZE + ICON_GAP + text_w + BADGE_PAD_X;
  const logical_h = pill_h + DOT_GAP + DOT_DIAM;

  const canvas = document.createElement("canvas");
  canvas.width = logical_w * RENDER_SCALE;
  canvas.height = logical_h * RENDER_SCALE;
  const ctx = canvas.getContext("2d")!;
  ctx.imageSmoothingEnabled = false;
  ctx.scale(RENDER_SCALE, RENDER_SCALE);

  // Pill background — a rect with fully-rounded ends (radius = half height).
  const r = pill_h / 2;
  ctx.fillStyle = "rgba(20,20,26,0.72)";
  ctx.beginPath();
  ctx.roundRect(0, 0, logical_w, pill_h, r);
  ctx.fill();

  // Icon circle, theme-colored, centered in the pill's own left end.
  const icon_cx = r;
  const icon_cy = pill_h / 2;
  ctx.fillStyle = theme.color;
  ctx.beginPath();
  ctx.arc(icon_cx, icon_cy, ICON_SIZE / 2, 0, Math.PI * 2);
  ctx.fill();
  ctx.font = ICON_FONT;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(theme.icon, icon_cx, icon_cy + 1);

  // Name, left-aligned right after the icon circle.
  ctx.font = FONT;
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  ctx.fillStyle = "#fff";
  ctx.fillText(name, BADGE_PAD_X + ICON_SIZE + ICON_GAP, pill_h / 2 + 1);

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

/** center.xy (page-local real units, poi_extractor.ts/osm_normalizer.ts)
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

/** The MAX_RENDERED POIs closest to map_center, deduped by id — a real
 * feature near a page boundary can come back from more than one page's own
 * bbox query (adjacent pages' queries legitimately overlap at the edges),
 * and without this it would render twice, once per page, at the exact same
 * position (`checkpoint/20260107-float`'s own PoiLayer hit the identical
 * case and solved it the same way — dedupe by id, keep whichever copy is
 * seen first). Ranked by distance rather than poi_extractor.ts's own rank
 * field: rank is "how notable is this in general", not "is this near the
 * camera" — a page-local top-by-rank cut (this file's previous version)
 * could easily fill the whole budget with far-away landmarks while
 * something two blocks from the camera never makes the list. */
const FAR_END_U2 = FAR_END * FAR_END;

function select_nearby_pois(
  poi_map: ReadonlyMap<IVec, readonly POI[]>,
  map_center: PVF3,
): { poi: POI; page_idx: IVec }[] {
  const seen = new Set<string>();
  const candidates: { poi: POI; page_idx: IVec; dist2: f64 }[] = [];

  for (const [page_idx, pois] of poi_map) {
    for (const poi of pois) {
      if (seen.has(poi.id)) continue;
      seen.add(poi.id);
      const dist2 = flat_dist2(poi_flat_pos(poi, page_idx), map_center);
      // Beyond FAR_END it would render at factor 0 anyway —
      // skip mounting it at all rather than paying for an invisible
      // AnchorNode + sprite + useFrame every one of those would otherwise
      // cost.
      if (dist2 > FAR_END_U2) continue;
      candidates.push({ poi, page_idx, dist2 });
    }
  }

  candidates.sort((a, b) => a.dist2 - b.dist2);
  return candidates.slice(0, MAX_RENDERED);
}

/** One POI's own absolute anchor — poi_flat_pos, then grounded to the real
 * terrain surface: center.z is always 0 coming from the bake
 * (osm_normalizer.ts's project_to_local_cell zeroes it deliberately, since a
 * POI's real height was never the bake's concern), so without this a badge
 * would sit at the page's own Z origin instead of floating above wherever
 * the ground actually is. */
function poi_anchor(poi: POI, page_idx: IVec): PVF3 | null {
  const flat = poi_flat_pos(poi, page_idx);
  return with_ele(flat, flat.i, flat.f);
}

function PoiBadge({ poi, page_idx }: { poi: POI; page_idx: IVec }) {
  const anchor = useMemo(() => poi_anchor(poi, page_idx), [poi, page_idx]);
  const built = useMemo(() => build_poi_badge_canvas(poi), [poi]);

  if (!anchor) return null;

  return (
    <AnchorNode i={anchor.i} f={anchor.f}>
      <Billboard built={built} base_z={GROUND_CLEARANCE_U} />
    </AnchorNode>
  );
}

export function PoiLayer() {
  const poi_map = usePoiStore((state) => state.poi_map);
  const map_center = useMapStore((state) => state.map_center);

  const selected = useMemo(() => (map_center ? select_nearby_pois(poi_map, map_center) : []), [poi_map, map_center]);

  return (
    <>
      {selected.map(({ poi, page_idx }) => <PoiBadge key={poi.id} poi={poi} page_idx={page_idx} />)}
    </>
  );
}
