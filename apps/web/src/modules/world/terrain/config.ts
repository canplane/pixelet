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
// Worst case (camera sitting at the far edge of its own page, looking
// outward) the render boundary is only (r-1) full pages away, i.e.
// (r-1)*L3_RANGE real units — at r=1 that's ~0u, well inside FOG_FAR and
// exactly the pop-in this constant was split out to fix in the first place.
// By that rule r should satisfy (r-1)*1024 >= FOG_FAR, and at FOG_FAR=900
// r=2 did — a worst-case reach of 1*L3_RANGE=1024u. (Same margin the
// original RENDER_RADIUS=8 @ 128u/page — (8-1)*128=896u, almost exactly
// FOG_FAR — already implied, re-derived at the 1024u page size.)
//
// At FOG_FAR=1800 it no longer does, and r stays 2 anyway. Geometry ends at
// 1024u, where fog is only ~33% opaque — so unlike at the intermediate
// FOG_FAR=1350 (~62%), the seam is not hidden and may well be visible on a
// hard zoom-out. It is left that way on purpose: raising r to satisfy the
// rule is measured to be the wrong lever — it mounts (2r+1)^2 pages, and a
// phone discarded the tab under r=3. Bring FOG_FAR down to meet r before
// moving r up to meet FOG_FAR.
export const RENDER_RADIUS: u32 = 2;
