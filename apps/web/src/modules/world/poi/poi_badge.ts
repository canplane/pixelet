// @pxl/web/modules/world/poi/poi_badge.ts

/**
 * One POI's drawn badge, built once and kept.
 *
 * This used to be a `useMemo` inside the badge component, which meant the
 * canvas and its texture were created when a badge entered the top
 * MAX_RENDERED and destroyed when it left — so panning across a dense area
 * created and destroyed browser-owned graphics objects at interaction rate.
 * The ledger sees that as `Δtex` swings of seventy or eighty in a single
 * frame, and Blink sees it as pre-finalizers to run: the 4.9 s stall
 * measured in
 * [discussions/20260910-where-the-allocation-goes.md] is that class of cost.
 *
 * Holding them by id fixes a second thing that looks unrelated and is not.
 * A collision pass needs each candidate's screen box before deciding what to
 * draw, and the box comes from `logical_w`/`logical_h` — which used to exist
 * only after the component had already been chosen and mounted. Measuring
 * three hundred candidates per tick by building three hundred canvases is
 * not an option; looking up three hundred cached sizes is free. Decluttering
 * and the texture churn turned out to share one prerequisite.
 *
 * Eviction runs on the same trigger the index rebuild does, because it is
 * the same event: a page leaving is exactly when a badge stops being
 * reachable. Nothing here evicts on its own schedule, so a badge cannot
 * disappear while something still refers to it.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import type { POI } from "@pxl/shared/bake/model/feature.ts";

import type { AtlasUV } from "../billboard/atlas.ts";
import { BillboardAtlas } from "../billboard/atlas.ts";
import { FONT_FAMILY, FONT_WEIGHT_REGULAR } from "../billboard/canvas_scale.ts";
import { resolve_poi_theme } from "./poi_theme.ts";

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
// Supersampling, and it should equal the renderer's own device pixel ratio.
//
// scene.tsx caps that at 2 (`dpr={aa_mode === "supersample" ? [1, 2] : [1, 1]}`)
// and distance_fade_far returns at most 1, so a billboard's drawn size never
// exceeds its logical size in CSS pixels — the fade only ever shrinks it.
// Two is therefore the exact requirement rather than a guess.
//
// It was 3, which is 2.25x the pixels for nothing: magFilter is Nearest for
// the up-close pixel read, and drawing at 3 to display at 2 is a
// minification, so it went through the mipmap chain and came out softer
// than drawing at 2 would have.
const RENDER_SCALE = 2;

// checkpoint/20260107-float's own "Anchor Dot" (poi-item.tsx) — a small dot
// under the pill marking the actual ground point, since the pill itself
// floats above and offset from it.
const DOT_DIAM = 6;
const DOT_GAP = 4;
const DOT_BORDER = 1;

/**
 * One sheet for every badge this layer draws, places and clusters alike.
 *
 * Shared so a single instanced draw is possible: two quads sampling
 * different textures cannot batch, whatever else is true of them.
 */
export const badge_atlas = new BillboardAtlas();

/** A badge, as where it is in the sheet and how big it should be drawn. No
 * canvas and no texture of its own — see atlas.ts. */
export type BuiltBadge = BadgeMetrics & { uv: AtlasUV };

const cache = new Map<string, BuiltBadge>();
const metrics_cache = new Map<string, BadgeMetrics>();

/**
 * A badge's size, without the badge.
 *
 * Everything the collision pass needs — it works in screen boxes, and a box
 * is a width and a height. Measured it is: three thousand candidates asking
 * for their size used to build three thousand canvases and textures to draw
 * eighty of them, at 0.09 ms each, which is where the 333 ms placement spike
 * came from. Nothing here touches a canvas except one shared measuring
 * context, and that one is never drawn into.
 *
 * Only `logical_w` actually varies today, since the rest is fixed by the
 * layout constants above. Returned as a record anyway: a badge that ever
 * wraps to two lines changes all four, and the callers should not have to be
 * revisited when it does.
 */
export type BadgeMetrics = {
  logical_w: f64;
  logical_h: f64;
  center_y: f64;
  collide_h: f64;
};

const PILL_H = ICON_SIZE + PAD_Y;
const PILL_R = PILL_H / 2;
const TEXT_X = PILL_R + ICON_SIZE / 2 + GAP;
const LOGICAL_H = PILL_H + DOT_GAP + DOT_DIAM;
const DOT_CY = PILL_H + DOT_GAP + DOT_DIAM / 2;

/** One context for every measurement there will ever be. Creating a canvas
 * per call was itself part of the cost being removed here. */
let $measure: CanvasRenderingContext2D | null = null;
function measure_ctx(): CanvasRenderingContext2D {
  if (!$measure) {
    $measure = document.createElement("canvas").getContext("2d")!;
    $measure.font = FONT;
  }
  return $measure;
}

/** Truncated by character count rather than wrap_text's own word-wrap — a map
 * badge reads as one short line, not a multi-line chat message. */
function display_name(poi: POI): string {
  return poi.name.length > NAME_MAX_CHARS ? `${poi.name.slice(0, NAME_MAX_CHARS - 1)}…` : poi.name;
}

/** Cache misses and what they cost, cumulative. `metrics` is a text
 * measurement; `builds` is a canvas, a bitmap and the texture the sprite
 * takes from it. Every candidate asks for the first and only what is drawn
 * asks for the second, so the gap between the two counts is the fix working. */
export const poi_badge_build_stats = { metrics: 0, builds: 0, ms: 0 };

export function poi_badge_metrics(poi: POI): BadgeMetrics {
  let m = metrics_cache.get(poi.id);
  if (!m) {
    const text_w = Math.ceil(measure_ctx().measureText(display_name(poi)).width);
    m = { logical_w: TEXT_X + text_w + GAP, logical_h: LOGICAL_H, center_y: DOT_CY, collide_h: PILL_H };
    poi_badge_build_stats.metrics++;
    metrics_cache.set(poi.id, m);
  }
  return m;
}

/** A single-line pill (colored icon circle + name) with an anchor dot
 * centered below it — the shape `checkpoint/20260107-float`'s own HTML
 * badge used (`poi-item.tsx`), redrawn as one flat canvas texture instead
 * of a DOM element (see this file's own top doc comment on why hover/
 * hover-lift/day-night from that version aren't reproduced here). */
function build_poi_badge(poi: POI): BuiltBadge | null {
  const theme = resolve_poi_theme(poi.category);
  const name = display_name(poi);
  const m = poi_badge_metrics(poi);
  const { logical_w } = m;

  const uv = badge_atlas.add(logical_w * RENDER_SCALE, LOGICAL_H * RENDER_SCALE, (ctx) => {
    ctx.imageSmoothingEnabled = false;
    ctx.scale(RENDER_SCALE, RENDER_SCALE);

    // Pill background — a rect with fully-rounded ends (radius = half height).
    ctx.fillStyle = "rgba(20,20,26,0.72)";
    ctx.beginPath();
    ctx.roundRect(0, 0, logical_w, PILL_H, PILL_R);
    ctx.fill();

    // Icon circle is concentric with the pill's own rounded left cap (same
    // center `r`, smaller radius) rather than padded off it — no separate
    // left-margin constant, `GAP` is the one deliberate breathing-room value
    // in this layout: icon-to-text and text-to-right-tip, equal on purpose.
    const icon_cx = PILL_R;
    const icon_cy = PILL_H / 2;
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
    ctx.fillText(name, TEXT_X, PILL_H / 2 + 1);

    // Anchor dot, centered under the whole pill.
    ctx.fillStyle = "rgba(255,255,255,0.6)";
    ctx.beginPath();
    ctx.arc(logical_w / 2, DOT_CY, DOT_DIAM / 2, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = "rgba(255,255,255,0.25)";
    ctx.lineWidth = DOT_BORDER;
    ctx.stroke();
  });
  return uv ? { ...m, uv } : null;
}

/** The badge for this POI, drawn on first ask and kept after. Only ask for
 * one that is going to be drawn — poi_badge_metrics answers everything the
 * selection pass needs without it. Null when the sheet is full, which the
 * caller shows as nothing rather than as a wrong rectangle. */
export function poi_badge(poi: POI): BuiltBadge | null {
  const hit = cache.get(poi.id);
  if (hit) return hit;

  const t0 = performance.now();
  const built = build_poi_badge(poi);
  poi_badge_build_stats.builds++;
  poi_badge_build_stats.ms += performance.now() - t0;

  // A full sheet gives back nothing rather than a wrong rectangle, and the
  // miss is not cached — the space it wanted may be free by the next ask.
  //
  // Silently drawing nothing is the wrong answer and is here on purpose,
  // because the right one is a policy this cache does not have yet: evict
  // the least recently used badge and take its rectangle. That is cheap and
  // entirely local — a badge is drawn from `poi_map`, which is already
  // resident, so evicting one costs the 0.09 ms to draw it again and never
  // a request. Whether it is needed at all is a question for the ledger's
  // `atlas ... % used`: eviction on page change may already keep the sheet
  // under its ceiling, in which case this branch is unreachable and should
  // say so instead.
  if (!built) return null;
  cache.set(poi.id, built);
  return built;
}

// ---- cluster badge ----
//
// A circle carrying a count, which is supercluster's own representation and
// the one a board of user posts wants: there is no single member worth
// naming, and the number *is* the information.
//
// Keyed by the count rather than by an id, which is the whole reason this is
// cheap. Every cluster of twelve looks alike, so the cache holds a few dozen
// entries for a whole session no matter how much world is crossed — and a
// cluster, unlike a POI, has no stable identity to key on anyway: it exists
// for one zoom level and dissolves at the next.

const CLUSTER_MIN_D = 30;
/** Added per decade of members, so a hundred reads as visibly bigger than
 * ten without a thousand becoming a disc. Area would be the honest encoding
 * and is unreadable at these sizes. */
const CLUSTER_D_PER_DECADE = 7;
const CLUSTER_FONT_PX = 12;

const cluster_cache = new Map<u32, BuiltBadge>();
const cluster_metrics_cache = new Map<u32, BadgeMetrics>();

function cluster_diameter(count: u32): f64 {
  return CLUSTER_MIN_D + CLUSTER_D_PER_DECADE * Math.log10(Math.max(1, count));
}

/** Capped, because four digits does not fit and the difference between 1200
 * and 1400 is not a difference anyone acts on. */
function cluster_label(count: u32): string {
  return count > 999 ? "999+" : String(count);
}

export function cluster_badge_metrics(count: u32): BadgeMetrics {
  let m = cluster_metrics_cache.get(count);
  if (!m) {
    const d = cluster_diameter(count);
    // The anchor is the circle's own centre, not its bottom edge: a cluster
    // marks a region rather than standing on a point, and the count should
    // sit over the place it counts.
    m = { logical_w: d, logical_h: d, center_y: d / 2, collide_h: d };
    cluster_metrics_cache.set(count, m);
  }
  return m;
}

export function cluster_badge(count: u32): BuiltBadge | null {
  let built = cluster_cache.get(count);
  if (built) return built;

  const t0 = performance.now();
  const m = cluster_badge_metrics(count);
  const d = m.logical_w;
  const r = d / 2;

  const uv = badge_atlas.add(d * RENDER_SCALE, d * RENDER_SCALE, (ctx) => {
    ctx.scale(RENDER_SCALE, RENDER_SCALE);
    ctx.fillStyle = "rgba(20,20,26,0.72)";
    ctx.beginPath();
    ctx.arc(r, r, r - 1, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = "rgba(255,255,255,0.35)";
    ctx.lineWidth = 1;
    ctx.stroke();

    ctx.font = `${FONT_WEIGHT_REGULAR} ${CLUSTER_FONT_PX}px ${FONT_FAMILY}`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillStyle = "#fff";
    ctx.fillText(cluster_label(count), r, r + 1);
  });
  poi_badge_build_stats.builds++;
  poi_badge_build_stats.ms += performance.now() - t0;
  if (!uv) return null;
  built = { ...m, uv };
  cluster_cache.set(count, built);
  return built;
}

/**
 * Drops every badge whose POI is no longer reachable.
 *
 * The canvas is released by dropping the reference — there is no GPU object
 * to dispose here, since the texture is owned by the material the sprite
 * builds from this canvas and dies with it. What this reclaims is the
 * bitmap, and what it prevents is a session-long map that grows with every
 * page ever visited.
 */
export function evict_poi_badges(live: ReadonlySet<string>): u32 {
  let dropped: u32 = 0;
  for (const [id, built] of cache) {
    if (live.has(id)) continue;
    // Its rectangle goes back to the sheet's free pool. Nothing is disposed
    // — there is one texture and it outlives every badge in it.
    badge_atlas.release(built.uv);
    cache.delete(id);
    dropped++;
  }
  // Metrics go with them. Four numbers rather than a bitmap, so this is about
  // not growing a map with every page ever visited rather than about memory
  // that matters — but a stale size is a wrong collision box, so they cannot
  // outlive the badge either.
  for (const id of metrics_cache.keys()) {
    if (!live.has(id)) metrics_cache.delete(id);
  }
  return dropped;
}

export function poi_badge_count(): u32 {
  return cache.size;
}
