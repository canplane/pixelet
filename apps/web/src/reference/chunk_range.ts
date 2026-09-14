// @pxl/web/reference/chunk_range.ts

/**
 * Which chunks of a page are in range of the anchor, as the TypeScript
 * decided it before the module mounted chunks itself
 * (crates/pxl-core/src/world/sched.rs's range mount): the reference
 * core_sched.test.ts holds the module's mounts to, anchor for anchor.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, PAGE_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { render_cfg } from "@/module/world/environment/config.ts";
import { rect_dist2 } from "@/module/world/frame/range.ts";

const PAGE_CHUNK_AXIS: u32 = PAGE_CELL_SPAN / CHUNK_CELL_SPAN;
const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN * U_PER_CELL;

export type ChunkOffsets = {
  key: string;
  /** Page-local chunk origin in canonical units. */
  base_x: f64;
  base_y: f64;
};

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
