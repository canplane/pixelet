// @pxl/web/modules/world/terrain/config.ts

import type { u32 } from "@xpute/core/kit/word.ts";

/** Tile-streaming tuning — see use_tile_fetcher.ts for how LOAD_RADIUS/
 * CLEANUP_RADIUS drive page load/eviction, and layer.tsx for how
 * RENDER_RADIUS bounds what gets mounted as scene objects. Three separate
 * knobs on purpose — each one trades off a different resource (API load,
 * client memory, render/draw-call cost) and none of them need to move
 * together. */

// preload radius around the current center page. Deliberately kept
// conservative: every page inside this radius is a live PostGIS query +
// bake on the API side (no cache/concurrency-limit/rate-limit there yet,
// see project notes), and page count grows as (2r+1)^2 — r=2 is 25 pages
// per full reload; r=3 would already be 49. This is the one knob NOT to
// casually raise.
export const LOAD_RADIUS: u32 = 2;

// cache eviction radius with hysteresis — wider than LOAD_RADIUS: pages
// between the two stay decoded in tile_map (cheap-ish: ~257KB/page — 4
// dense PAGE_CELL_SPAN^2 ground channels dominate that, ele is only
// ~800B) so a small pan back doesn't re-fetch over the network. A
// standalone value (not LOAD_RADIUS*2) since the two tune independently:
// this one only costs client memory (every page here was already
// fetched), never a new PostGIS/bake request, so it can afford to be far
// more generous than LOAD_RADIUS — r=8 is 289 pages, ~74MB, comfortably
// inside a mobile browser's budget.
export const CLEANUP_RADIUS: u32 = 8;

// how far out layer.tsx actually mounts pages as scene objects (draw
// calls, per-chunk LOD useFrame). Used to be pinned to LOAD_RADIUS — a
// much tighter box than what's already sitting in the client-side cache
// for free — which made cached-but-unmounted terrain pop in/out right at
// that edge, plainly visible up close and nowhere near where fog/far-clip
// (world/render/config.ts's render_cfg) actually faded anything out. The
// LOD system already collapses far chunks to a handful of coarse
// instances, so mounting everything already-cached costs much less than
// mounting that same page count at full detail would. Tied to
// CLEANUP_RADIUS for now (render = everything you're already holding in
// memory, no separate narrower cutoff) — kept as its own named constant
// since that's a starting point, not a permanent identity; nothing else
// assumes the two stay equal.
export const RENDER_RADIUS: u32 = CLEANUP_RADIUS;
