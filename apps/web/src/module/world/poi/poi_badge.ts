// @pxl/web/module/world/poi/poi_badge.ts

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
 * [issue/20260910-where-the-allocation-goes.md] is that class of cost.
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

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";
import { Vector } from "@xpute/core/ds/arena.ts";
import { I64_KEYS, IndexedMap, type KeyTraits, U32_KEYS } from "@xpute/core/ds/map.ts";

import type { PoiPage } from "@pxl/shared/bake/format.ts";

import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

import { NO_SLOT } from "../billboard/atlas.ts";
import { billboard_sheet } from "../billboard/sheet.ts";
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
// scene.tsx caps that at 2 (`dpr={[1, 2]}`)
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
 * different textures cannot batch, whatever else is true of them. It is
 * billboard/sheet.ts's, which the chat bubble draws into too.
 */
export const badge_atlas = billboard_sheet;

const PILL_H = ICON_SIZE + PAD_Y;
const PILL_R = PILL_H / 2;
const TEXT_X = PILL_R + ICON_SIZE / 2 + GAP;
const LOGICAL_H = PILL_H + DOT_GAP + DOT_DIAM;
const DOT_CY = PILL_H + DOT_GAP + DOT_DIAM / 2;

/**
 * A badge's size, without the badge — everything the collision pass needs,
 * since it works in screen boxes and a box is a width and a height. Only the
 * width varies: it is measured text, and the measurement is cached below.
 * The rest is fixed by the layout constants above, and named here so a
 * consumer reads the layout and not a number.
 */
export const BADGE_LOGICAL_H: f64 = LOGICAL_H;
export const BADGE_CENTER_Y: f64 = DOT_CY;
export const BADGE_COLLIDE_H: f64 = PILL_H;

/**
 * What is known about each badge: its measured width, and the atlas slot it
 * is drawn in once it has been (NO_SLOT until then). Keyed in the core
 * memory — a place by its id hash, the page's own lane (format.ts), a
 * cluster by its count.
 *
 * Measuring is cheap and building is not: three thousand candidates asking
 * for their size used to build three thousand canvases and textures to draw
 * eighty of them, at 0.09 ms each, which is where the 333 ms placement spike
 * came from. Nothing here touches a canvas except one shared measuring
 * context, and that one is never drawn into.
 */
class BadgeTable<K extends number | bigint> {
  readonly index: IndexedMap<K>;
  readonly logical_w = new Vector(Float64Array, { init_cap: 256, alloc: CORE_ALLOC });
  readonly slot = new Vector(Int32Array, { init_cap: 256, alloc: CORE_ALLOC });
  /** Badges drawn in the sheet. */
  built: u32 = 0;

  constructor(traits: KeyTraits<K>) {
    this.index = new IndexedMap<K>(traits, 256, CORE_ALLOC);
  }

  insert(key: K, logical_w: f64): u32 {
    const pos = this.index.insert(key);
    this.logical_w.set(pos, logical_w);
    this.slot.set(pos, NO_SLOT);
    return pos;
  }

  remove(pos: u32): void {
    if (this.slot.mem[pos] !== NO_SLOT) {
      badge_atlas.release(this.slot.mem[pos]);
      this.built--;
    }
    const from = this.index.remove_at(pos);
    if (from !== pos) {
      this.logical_w.mem[pos] = this.logical_w.mem[from];
      this.slot.mem[pos] = this.slot.mem[from];
    }
    this.logical_w.truncate(from);
    this.slot.truncate(from);
  }
}

const places = new BadgeTable<bigint>(I64_KEYS);
const clusters = new BadgeTable<number>(U32_KEYS);

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
function display_name(page: PoiPage, rec: u32): string {
  const name = page.name_of(rec);
  return name.length > NAME_MAX_CHARS ? `${name.slice(0, NAME_MAX_CHARS - 1)}…` : name;
}

/** Cache misses and what they cost, cumulative. `metrics` is a text
 * measurement — asked only for what is drawn, since a candidate's width
 * is estimated; `builds` is a canvas, a bitmap and the upload. */
export const poi_badge_build_stats = { metrics: 0, builds: 0, ms: 0 };

/**
 * Text width from the characters, without the canvas.
 *
 * Every candidate a placement offers to the screen needs a width for the
 * collision test, and a placement over a new area offers a couple of
 * thousand. `measureText` on a phone is around a millisecond for a Korean
 * name — shaping — so measuring every candidate was a third of a second
 * of main-thread time per new area, in units the budget could only take
 * one of a frame (2026-09-13). An estimate from the character classes is
 * within a glyph or two of the truth, which the collision test does not
 * notice, and costs nothing. The font's own advances are read once for
 * each class rather than assumed, so a font change does not drift it.
 * What is drawn is measured exactly, at build, and the cache is corrected
 * to that.
 */
let $advance: { cjk: f64; lower: f64; upper: f64; digit: f64; space: f64 } | null = null;
function advances() {
  if (!$advance) {
    const ctx = measure_ctx();
    $advance = {
      cjk: ctx.measureText("가나다라마바사아자차").width / 10,
      lower: ctx.measureText("abcdefghijklmnopqrstuvwxyz").width / 26,
      upper: ctx.measureText("ABCDEFGHIJKLMNOPQRSTUVWXYZ").width / 26,
      digit: ctx.measureText("0123456789").width / 10,
      space: ctx.measureText(" ").width,
    };
  }
  return $advance;
}

function estimate_text_w(text: string): f64 {
  const a = advances();
  let w = 0;
  for (const ch of text) {
    const c = ch.codePointAt(0)!;
    if (c === 0x20) w += a.space;
    else if (c >= 0x30 && c <= 0x39) w += a.digit;
    else if (c >= 0x41 && c <= 0x5a) w += a.upper;
    else if (c < 0x80) w += a.lower;
    // Hangul, CJK ideographs, kana, fullwidth forms: one em each.
    else if (c >= 0x1100) w += a.cjk;
    else w += a.upper;
  }
  return Math.ceil(w);
}

/** The badge's width in logical pixels: estimated on first ask, exact once
 * the badge has been drawn. */
export function poi_badge_logical_w(page: PoiPage, rec: u32): f64 {
  const pos = places.index.find(page.id_hash[rec]);
  if (pos >= 0) return places.logical_w.mem[pos];
  return places.logical_w.mem[places.insert(page.id_hash[rec], TEXT_X + estimate_text_w(display_name(page, rec)) + GAP)];
}

/** The pill every badge in this layer stands on: a rect with fully-rounded
 * ends (radius = half height). One function so a place and a cluster cannot
 * drift into two designs. */
function draw_pill(ctx: CanvasRenderingContext2D, logical_w: f64): void {
  ctx.fillStyle = "rgba(20,20,26,0.72)";
  ctx.beginPath();
  ctx.roundRect(0, 0, logical_w, PILL_H, PILL_R);
  ctx.fill();
}

/** The anchor dot, centred under the pill, marking the ground point the
 * pill floats above. */
function draw_anchor_dot(ctx: CanvasRenderingContext2D, logical_w: f64): void {
  ctx.fillStyle = "rgba(255,255,255,0.6)";
  ctx.beginPath();
  ctx.arc(logical_w / 2, DOT_CY, DOT_DIAM / 2, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = "rgba(255,255,255,0.25)";
  ctx.lineWidth = DOT_BORDER;
  ctx.stroke();
}

/** A single-line pill (colored icon circle + name) with an anchor dot
 * centered below it — the shape `checkpoint/20260107-float`'s own HTML
 * badge used (`poi-item.tsx`), redrawn as one flat canvas texture instead
 * of a DOM element (see this file's own top doc comment on why hover/
 * hover-lift/day-night from that version aren't reproduced here). */
function build_poi_badge(page: PoiPage, rec: u32, logical_w: f64): i32 {
  const theme = resolve_poi_theme(page.category_of(rec));
  const name = display_name(page, rec);

  return badge_atlas.add(logical_w * RENDER_SCALE, LOGICAL_H * RENDER_SCALE, (ctx) => {
    ctx.imageSmoothingEnabled = false;
    ctx.scale(RENDER_SCALE, RENDER_SCALE);

    draw_pill(ctx, logical_w);

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

    draw_anchor_dot(ctx, logical_w);
  });
}

/** The atlas slot of this POI's badge, drawn on first ask and kept after.
 * Only ask for one that is going to be drawn — poi_badge_logical_w answers
 * everything the selection pass needs without it. NO_SLOT when the sheet is
 * full, which the caller shows as nothing rather than as a wrong rectangle. */
export function poi_badge_slot(page: PoiPage, rec: u32): i32 {
  poi_badge_logical_w(page, rec);
  const pos = places.index.find(page.id_hash[rec]);
  if (places.slot.mem[pos] !== NO_SLOT) return places.slot.mem[pos];

  const t0 = performance.now();
  // Exact, now that it is drawn: the pill has to fit the text, and the
  // sprite has to be the pill's size.
  const logical_w = TEXT_X + Math.ceil(measure_ctx().measureText(display_name(page, rec)).width) + GAP;
  places.logical_w.mem[pos] = logical_w;
  poi_badge_build_stats.metrics++;
  const slot = build_poi_badge(page, rec, logical_w);
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
  if (slot === NO_SLOT) return NO_SLOT;
  places.slot.mem[pos] = slot;
  places.built++;
  return slot;
}

// ---- cluster badge ----
//
// A count where a name would be. It is the same pill a place gets, at the
// same height, with the same dot under it marking the lattice cell the
// count stands over — one design for the layer, so a badge that gathers
// its neighbours reads as a badge and not as a different kind of thing.
// There is no icon: a cluster has no category, and the number is the
// information (supercluster's own representation, and the one a board of
// user posts wants).
//
// Keyed by the count rather than by an id, which is the whole reason this is
// cheap. Every cluster of twelve looks alike, so the cache holds a few dozen
// entries for a whole session no matter how much world is crossed — and a
// cluster, unlike a POI, has no stable identity to key on anyway: it exists
// for one zoom level and dissolves at the next.

/** Horizontal breathing room either side of the count. Chosen so a single
 * digit fills exactly a circle of the pill's height, and anything longer
 * stretches it into a pill — the count's own length carries its magnitude,
 * which is why the diameter no longer grows per decade. */
const CLUSTER_PAD_X = 10;

/** Capped, because four digits does not fit and the difference between 1200
 * and 1400 is not a difference anyone acts on. */
function cluster_label(count: u32): string {
  return count > 999 ? "999+" : String(count);
}

export function cluster_badge_logical_w(count: u32): f64 {
  const pos = clusters.index.find(count);
  if (pos >= 0) return clusters.logical_w.mem[pos];
  const text_w = Math.ceil(measure_ctx().measureText(cluster_label(count)).width);
  return clusters.logical_w.mem[clusters.insert(count, Math.max(PILL_H, text_w + CLUSTER_PAD_X * 2))];
}

export function cluster_badge_slot(count: u32): i32 {
  const logical_w = cluster_badge_logical_w(count);
  const pos = clusters.index.find(count);
  if (clusters.slot.mem[pos] !== NO_SLOT) return clusters.slot.mem[pos];

  const t0 = performance.now();
  const slot = badge_atlas.add(logical_w * RENDER_SCALE, LOGICAL_H * RENDER_SCALE, (ctx) => {
    ctx.imageSmoothingEnabled = false;
    ctx.scale(RENDER_SCALE, RENDER_SCALE);

    draw_pill(ctx, logical_w);

    ctx.font = FONT;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillStyle = "#fff";
    ctx.fillText(cluster_label(count), logical_w / 2, PILL_H / 2 + 1);

    draw_anchor_dot(ctx, logical_w);
  });
  poi_badge_build_stats.builds++;
  poi_badge_build_stats.ms += performance.now() - t0;
  if (slot === NO_SLOT) return NO_SLOT;
  clusters.slot.mem[pos] = slot;
  clusters.built++;
  return slot;
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
export function evict_poi_badges(live: IndexedMap<bigint>): u32 {
  let dropped: u32 = 0;
  // Backwards, since a removal moves the last position into the hole. A
  // badge's rectangle goes back to the sheet's free pool and nothing is
  // disposed — there is one texture and it outlives every badge in it. The
  // measured width goes with it: a stale size is a wrong collision box.
  for (let pos = places.index.size - 1; pos >= 0; pos--) {
    if (live.find(places.index.key_at(pos)) >= 0) continue;
    if (places.slot.mem[pos] !== NO_SLOT) dropped++;
    places.remove(pos);
  }
  // The only moment the sheet's live set shrinks, so the moment to see
  // whether it still needs to be as large as the densest area made it.
  if (dropped > 0) badge_atlas.compact();
  return dropped;
}

export function poi_badge_count(): u32 {
  return places.built;
}
