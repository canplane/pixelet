// @pxl/web/module/world/terrain/config.ts

/**
 * Tile streaming, in world units.
 *
 * The page stays the unit of transfer and storage: one PostGIS query and bake
 * on the API side, one ~4MB decode, one `tile_map` key, one eviction. What is
 * derived rather than chosen is *which* pages: the nearest RESIDENT_PAGES to
 * the view anchor (module/world/frame/range.ts's pages_to_hold), which is at once
 * what gets fetched and what stays decoded. One set, so a page can never be
 * requested and then dropped from under its own in-flight request — every
 * page here is a live query against an API with no cache or rate limit of its
 * own yet.
 *
 * The three independent page radii this replaces (fetch 1, evict 3, render 2,
 * all Chebyshev around the page the view centre stood in) could not agree with
 * a fog distance in world units, and all of them moved in whole-page steps as
 * that centre page changed. See issue/20260912-what-to-render-when.md.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { ELE_PADDED_SPAN, PAGE_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import { GROUND_CH_KEYS } from "@pxl/shared/bake/format.ts";

import { PAGE_SECTION_BYTES } from "@/wasm/core_pool.ts";
import { KEEP_DIST as KEEP, PAGE_MARGIN as MARGIN, RESIDENT_PAGES_CAP } from "./config_spec.ts";

/** What one resident page takes of the page section: its ground lanes and
 * its elevation block, each rounded up to the buddy's power of two. */
function pow2_ceil(bytes: u32): u32 {
  return 1 << Math.ceil(Math.log2(Math.max(1, bytes)));
}
const PAGE_BLOCK_BYTES: u32 = GROUND_CH_KEYS.length * pow2_ceil(PAGE_CELL_SPAN * PAGE_CELL_SPAN) + pow2_ceil(ELE_PADDED_SPAN * ELE_PADDED_SPAN * 2);

const PAGE_MARGIN: u32 = MARGIN;

/**
 * How many pages stay decoded: the nearest this many, and the rest are
 * dropped. A count, not a radius, because the thing worth bounding is memory
 * — one decoded page is ~4MB and a tab has been lost to holding too many —
 * and a count bounds it directly at any zoom or range.
 *
 * It has to be comfortably more than the ~21 the range itself fetches, or
 * panning out of a page and back refetches it, and a page with no tile is not
 * merely coarse: it is not mounted at all, so its whole square vanishes at
 * once. A radius of a quarter page was tried here first and did exactly that.
 * 32 pages is ~129MB, against the 49 (~197MB) that discarded the tab on an
 * iPhone 13 mini and the 9 (~36MB) the old fetch window held.
 *
 * Never more than the core memory's page section holds, less the margin:
 * a page past the section spills where the module cannot read it, and its
 * chunks are never built — so the count is bounded here by construction
 * rather than trusted to agree with core_pool.ts's layout.
 */
export const RESIDENT_PAGES: u32 = Math.min(RESIDENT_PAGES_CAP, Math.floor(PAGE_SECTION_BYTES / PAGE_BLOCK_BYTES) - PAGE_MARGIN);

/** How far to look when picking those pages. Only wide enough that the
 * nearest RESIDENT_PAGES are always available to pick (39 at worst), since
 * the count is what actually decides; a page beyond this is further than the
 * 32nd nearest can ever be. */
export const KEEP_DIST: f64 = KEEP;
