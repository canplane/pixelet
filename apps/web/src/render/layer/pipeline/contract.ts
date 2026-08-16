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
   */
  paint(key: ChunkKey, target: ChunkVolume): boolean;
}

/** Registration order is override priority: later layers win. */
export type WorldLayerStack = readonly WorldLayer[];
