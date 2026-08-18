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
// React/Three object and useFrame each. Kept independent of LOAD_RADIUS
// (too tight — makes cached-but-unmounted terrain visibly pop in/out well
// short of where fog actually fades anything out) and of CLEANUP_RADIUS
// (too loose on its own — up to 49 pages x 256 columns/page would need the
// real-distance filter, global reveal/LOD budgets, and batching to bound
// that cost regardless).
//
// Worst case (camera at the far edge of its own page, looking outward) the
// render boundary is only (r-1) full pages away, i.e. (r-1)*L3_RANGE real
// units — for that boundary to sit at or past FOG_FAR (world/environment/
// config.ts's render_cfg.FOG_FAR) rather than popping in visibly, r must
// satisfy (r-1)*1024 >= FOG_FAR. At the current FOG_FAR=1800 that would need
// r=3, but r stays at 2 anyway: geometry then ends at 1024u, where fog is
// only ~33% opaque, so the seam can be visible on a hard zoom-out — left
// that way on purpose, because raising r is the wrong lever regardless: it
// mounts (2r+1)^2 pages, and r=3 crashed the tab on a real device (iPhone 13
// mini). Bring FOG_FAR down to meet r before moving r up to meet FOG_FAR.
export const RENDER_RADIUS: u32 = 2;
