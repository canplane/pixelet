// @pxl/web/render/chunk_range.ts

/** Direct 64x64 chunk enumeration inside one L3 page. */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, PAGE_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { render_cfg } from "@/modules/world/environment/config.ts";
import type { ChunkOffsets } from "./layer/pipeline/chunk_bucket.ts";

const PAGE_CHUNK_AXIS: u32 = PAGE_CELL_SPAN / CHUNK_CELL_SPAN;
const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN * U_PER_CELL;
const FOG_FAR2: f64 = render_cfg.FOG_FAR * render_cfg.FOG_FAR;

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
 * Enumerates renderer chunks directly — at most 16x16 L2 chunk candidates
 * per page, with no finer-grained intermediate list ever built, filtered,
 * sorted, revealed, or regrouped. Chunks whose rectangle merely intersects
 * the visible fog circle are kept so their near edge cannot clip before it
 * fades out.
 */
export function in_range_chunks(page_base: IVec, map_center: IVec | null): ChunkOffsets[] {
  if (map_center === null) return [];

  const page_delta = ivec.sub(page_base, map_center);
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
      if (rect_dist2(x0, y0, x1, y1) > FOG_FAR2) continue;

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
