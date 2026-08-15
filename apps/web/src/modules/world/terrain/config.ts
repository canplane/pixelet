// @pxl/web/modules/world/terrain/config.ts

import type { u32 } from "@xpute/core/kit/word.ts";

/** Tile-streaming tuning — see use_tile_fetcher.ts for how LOAD_RADIUS/
 * CLEANUP_RADIUS drive page load/eviction, and chunk_stage.tsx for how
 * RENDER_RADIUS bounds what gets mounted as scene objects. Three separate
 * knobs on purpose — each one trades off a different resource (API load,
 * client memory, render/draw-call cost) and none of them need to move
 * together. */

// preload radius around the current center page, in PAGES (L3_IDX units —
// see use_tile_fetcher.ts), not chunks. Page count grows as (2r+1)^2, and
// each page is a live PostGIS query + bake on the API side (no cache/
// concurrency-limit/rate-limit there yet, see project notes) — the one
// knob to keep conservative. r=1 reaches L3_RANGE=1024u out.
export const LOAD_RADIUS: u32 = 1;

// cache eviction radius with hysteresis — wider than LOAD_RADIUS: pages
// between the two stay decoded in tile_map so a small pan back doesn't
// re-fetch over the network. A standalone value (not LOAD_RADIUS*2) since
// the two tune independently: this one only costs client memory (every
// page here was already fetched), never a new PostGIS/bake request. r=3 is
// 49 pages; measured live, one decoded page is ~4.03MB, so r=3 holds
// ~197MB decoded — a budget worth watching on low-end mobile once the
// chunk-build path is frame-budgeted instead of synchronous (see project
// notes).
export const CLEANUP_RADIUS: u32 = 3;

// how far out chunk_stage.tsx lets pages contribute to the renderer. A page
// has up to 256 renderer-owned L2 columns before the real-distance filter;
// those columns share one central LOD scheduler instead of owning one
// React/Three object and useFrame each. Used to be
// pinned to LOAD_RADIUS — a much tighter box than what's already sitting
// in the client-side cache for free — which made cached-but-unmounted
// terrain pop in/out right at that edge, plainly visible up close and
// nowhere near where fog (world/environment/config.ts's render_cfg.FOG_FAR=900)
// actually faded anything out; RENDER_RADIUS got tied to CLEANUP_RADIUS to
// fix that. Decoupled again here — CLEANUP_RADIUS=3 now means up to 49
// pages x 256 columns/page if render followed it all the way out; the
// real-distance filter, global reveal/LOD budgets, and batching are all
// required because page radius alone is too coarse to bound that cost.
//
// r=2, not r=1: worst case (camera sitting at the far edge of its own
// page, looking outward) the render boundary is only (r-1) full pages
// away, i.e. (r-1)*L3_RANGE real units — at r=1 that's ~0u, well inside
// FOG_FAR and exactly the pop-in this constant was split out to fix in
// the first place. r=2 guarantees a worst-case reach of 1*L3_RANGE=1024u,
// comfortably past FOG_FAR=900 in every camera position, not just the
// average one. (This is the same margin the original RENDER_RADIUS=8 @
// 128u/page — (8-1)*128=896u, almost exactly FOG_FAR — already implied;
// r=2 here is that same margin re-derived at the new 1024u page size, not
// an arbitrary new number.)
export const RENDER_RADIUS: u32 = 2;
