// @pxl/web/reference/contract.ts

/**
 * Adapter contract between source-specific world stores and the renderer's
 * temporary L2 64x64 column. Source-side storage (dense terrain pages,
 * sparse object octrees) stays untouched by this contract. What is drawn
 * and hit-tested is composed in the module from the same sources; the
 * layers here are the TypeScript reference the module is held to.
 */

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { ChunkVolume } from "@/reference/volume.ts";

/**
 * One renderer-owned 64x64 chunk inside an L3 terrain page. `page_off` is
 * 64-aligned on X/Y and always has Z=0. It identifies an L2 XY column;
 * occupied signed L2 Z bands are discovered during composition.
 */
export type ChunkKey = {
  page_idx: IVec;
  page_off: IVec;
};

export interface WorldLayer {
  readonly name: string;

  /**
   * Paints directly into the shared signed-band target. Later registered
   * layers overwrite earlier layers at the same voxel. Returns true when the
   * layer contributed at least one voxel.
   *
   * `opts.for_raycast` tells a layer this composite is for hit-testing,
   * not the drawn build — see ObjectWorldLayer's own use of it in
   * reference/object_layer.ts. A layer that paints a cosmetic,
   * render-only exclusion (an object hover-hidden for the eraser tool's
   * preview, object/store.ts's set_virtual_hide) must still paint the real
   * thing for a raycast pass, or the raycaster can no longer hit what the
   * user is pointing at — hover clears, the exclusion lifts, the raycaster
   * hits it again, hover sets, the exclusion reapplies — an infinite loop
   * (confirmed live, "Maximum update depth exceeded" 2026-09-03).
   */
  paint(key: ChunkKey, target: ChunkVolume, opts?: { for_raycast?: boolean }): boolean;
}

/** Registration order is override priority: later layers win. */
export type WorldLayerStack = readonly WorldLayer[];
