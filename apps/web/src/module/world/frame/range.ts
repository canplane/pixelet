// @pxl/web/module/world/frame/range.ts

/** What a view needs: which L3 pages, and which 64x64 chunks inside one. */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, PAGE_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { FROM_L3_IDX, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { render_cfg } from "@/module/world/environment/config.ts";
import type { ChunkOffsets } from "@/render/mesh/chunk_bucket.ts";

const PAGE_CHUNK_AXIS: u32 = PAGE_CELL_SPAN / CHUNK_CELL_SPAN;
const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN * U_PER_CELL;
/** One page's span in world units. */
export const PAGE_SPAN: f64 = PAGE_CELL_SPAN * U_PER_CELL;
/**
 * How far past the render range something already built is kept.
 *
 * Building is decided at CHUNK_RENDER_DIST and discarding only out here, so
 * the two thresholds cannot trade a chunk back and forth. Without it, a page
 * whose nearest corner sat 1680u away against a 1664u range was unmounted
 * with every chunk in it, and panning 16u back rebuilt all of them from the
 * scan up: measured 2026-09-12, and it reads on screen as the side you are
 * moving away from being cut off and redrawn
 * (issue/20260912-what-to-render-when.md).
 *
 * Four chunks, which is far more than a frame of panning can cross.
 */
export const RANGE_HYSTERESIS: f64 = 4 * CHUNK_WORLD_SPAN;
const CHUNK_ALIGN = ~(BigInt(CHUNK_CELL_SPAN) - 1n);

if (!Number.isInteger(PAGE_CHUNK_AXIS)) {
  throw new Error("render: an L3 page must contain a whole number of chunks");
}

/** Squared distance from the origin to an axis-aligned XY rectangle. */
function rect_dist2(x0: f64, y0: f64, x1: f64, y1: f64): f64 {
  const dx = 0 < x0 ? x0 : 0 > x1 ? -x1 : 0;
  const dy = 0 < y0 ? y0 : 0 > y1 ? -y1 : 0;
  return dx * dx + dy * dy;
}

/**
 * The position a view's ranges are measured from: the view centre, snapped
 * down to a chunk.
 *
 * Chunk-aligned and not page-aligned, which is the whole point. Keyed on the
 * page the centre happened to be in, every range stepped by a whole page as a
 * boundary was crossed, and a page's worth of terrain entered or left in one
 * frame (issue/20260912-what-to-render-when.md). Snapped rather than
 * exact so that moving within one chunk re-derives nothing.
 */
export function range_anchor_of(center: IVec): IVec {
  return ivec.gather(ivec.X(center) & CHUNK_ALIGN, ivec.Y(center) & CHUNK_ALIGN, 0n);
}

/** Squared distance from `anchor` to the nearest point of `page_idx`'s own
 * square. Zero while the anchor stands inside that page. */
export function page_dist2(anchor: IVec, page_idx: IVec): f64 {
  const delta = ivec.sub(FROM_L3_IDX(page_idx), anchor);
  const x0 = ivec.to_real(ivec.X(delta));
  const y0 = ivec.to_real(ivec.Y(delta));
  return rect_dist2(x0, y0, x0 + PAGE_SPAN, y0 + PAGE_SPAN);
}

/**
 * Every page whose square comes within `radius` of `anchor`, nearest first.
 *
 * The page stays the streaming unit — one query, one bake, one decode, one
 * cache key, one eviction — and this is the *set* of them, derived from a
 * distance rather than chosen as a page radius of its own. A page enters when
 * the disc reaches it, which happens continuously as the view moves; a
 * Chebyshev radius around the centre's own page could only change in whole
 * pages at a time.
 */
export function pages_in_radius(anchor: IVec, radius: f64): { page_idx: IVec; dist2: f64 }[] {
  const center_page = TO_XY(L3_IDX(anchor));
  const span = Math.ceil(radius / PAGE_SPAN) + 1;
  const radius2 = radius * radius;
  const out: { page_idx: IVec; dist2: f64 }[] = [];
  for (let dy = -span; dy <= span; dy++) {
    for (let dx = -span; dx <= span; dx++) {
      const page_idx = ivec.add(center_page, ivec.gather(BigInt(dx), BigInt(dy), 0n));
      const dist2 = page_dist2(anchor, page_idx);
      if (dist2 <= radius2) out.push({ page_idx, dist2 });
    }
  }
  out.sort((a, b) => a.dist2 - b.dist2);
  return out;
}

/**
 * The pages a view holds: the nearest `count` of those whose square comes
 * within `radius`, nearest first.
 *
 * One definition serves both what is fetched and what is kept, and that is
 * the point rather than a convenience. When the two were separate — fetch a
 * radius, keep a count — a page could be requested and then fall out of the
 * fetch set a moment later while its request was still in flight, and
 * fetch_scheduler sheds demands its category no longer wants. Panning
 * therefore cancelled its own tiles before they arrived, the page stayed
 * unmounted because a page with no tile is not mounted at all, and the view
 * lost a square of terrain on whichever side it was moving away from
 * (issue/20260912-what-to-render-when.md).
 *
 * Because membership now changes only when a page crosses the nearest-`count`
 * ranking, a page being fetched is by construction one worth keeping.
 */
export function pages_to_hold(anchor: IVec, radius: f64, count: u32): { page_idx: IVec; dist2: f64 }[] {
  return pages_in_radius(anchor, radius).slice(0, count);
}

/**
 * How far past the held set a page whose request is already in flight is
 * still asked for. Without it, the 32nd-nearest page changed with every 64u
 * of panning, and fetch_scheduler yields what a category no longer wants —
 * so panning aborted its own requests at the edge of the set and refetched
 * them a moment later (200 terrain aborts in one session, 2026-09-12). One
 * page: a pan cannot carry a page that far out before its request settles,
 * while a search jump, which moves by many pages, still yields as designed.
 */
export const FETCH_HYSTERESIS: f64 = PAGE_SPAN;

/**
 * Fetch order for a page, as a scheduler priority: its distance in whole
 * pages, doubled so a second subsystem can interleave at equal distance
 * (POI sits one behind terrain — see use_poi_fetcher.ts).
 */
export function page_priority(dist2: f64): number {
  return Math.round(Math.sqrt(dist2) / PAGE_SPAN) * 2;
}

/**
 * Enumerates renderer chunks directly — at most 16x16 L2 chunk candidates
 * per page, with no finer-grained intermediate list ever built, filtered,
 * sorted, revealed, or regrouped. Chunks whose rectangle merely intersects
 * the render range are kept so their near edge cannot clip before fog has
 * hidden it.
 */
export function in_range_chunks(page_base: IVec, anchor: IVec | null, radius: f64 = render_cfg.CHUNK_RENDER_DIST): ChunkOffsets[] {
  if (anchor === null) return [];
  const radius2 = radius * radius;

  const page_delta = ivec.sub(page_base, anchor);
  const page_x = ivec.to_real(ivec.X(page_delta));
  const page_y = ivec.to_real(ivec.Y(page_delta));
  const out: Array<ChunkOffsets & { dist2: f64 }> = [];

  for (let by: u32 = 0; by < PAGE_CHUNK_AXIS; by++) {
    const base_y = by * CHUNK_CELL_SPAN;
    const y0 = page_y + base_y * U_PER_CELL;
    const y1 = y0 + CHUNK_WORLD_SPAN;
    for (let bx: u32 = 0; bx < PAGE_CHUNK_AXIS; bx++) {
      const base_x = bx * CHUNK_CELL_SPAN;
      const x0 = page_x + base_x * U_PER_CELL;
      const x1 = x0 + CHUNK_WORLD_SPAN;
      if (rect_dist2(x0, y0, x1, y1) > radius2) continue;

      const center_x = x0 + CHUNK_WORLD_SPAN * 0.5;
      const center_y = y0 + CHUNK_WORLD_SPAN * 0.5;
      out.push({
        key: `${base_x},${base_y}`,
        base_x,
        base_y,
        dist2: center_x * center_x + center_y * center_y,
      });
    }
  }

  out.sort((a, b) => a.dist2 - b.dist2);
  return out.map(({ dist2: _dist2, ...offset }) => offset);
}
