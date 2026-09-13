// @pxl/web/render/layer/pipeline/contract.ts

/**
 * Adapter contract between source-specific world stores and the renderer's
 * temporary L2 64x64 column. Source-side storage (dense terrain pages,
 * sparse object octrees) stays untouched by this contract.
 */

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { ChunkVolume } from "./volume.ts";

/**
 * One renderer-owned 64x64 chunk inside an L3 terrain page. `page_off` is
 * 64-aligned on X/Y and always has Z=0. It identifies an L2 XY column;
 * occupied signed L2 Z bands are discovered during composition.
 */
export type ChunkKey = {
  page_idx: IVec;
  page_off: IVec;
};

/**
 * Reference/value tuple describing everything that can change a layer's
 * output for one key. Tuple items are compared with Object.is. A new layer
 * owns this contract itself; compositor.ts never imports concrete stores or
 * grows another hard-coded invalidation condition.
 */
export type WorldLayerRevision = readonly unknown[];

export interface WorldLayer {
  readonly name: string;

  /** Cheap, synchronous cache identity for this layer at `key`. */
  revision(key: ChunkKey): WorldLayerRevision;

  /**
   * Paints directly into the shared signed-band target. Later registered
   * layers overwrite earlier layers at the same voxel. Returns true when the
   * layer contributed at least one voxel.
   *
   * `opts.for_raycast` tells a layer this composite is for hit-testing
   * (compositor.ts's own with_raycast_composite), not the real GPU-mesh
   * build (with_temporary_composite) — see ObjectWorldLayer's own use of it
   * in render/layer/object/adapter.ts. A layer that paints a cosmetic,
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
