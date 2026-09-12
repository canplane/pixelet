// @pxl/web/render/layer/pipeline/contract.ts

/**
 * Adapter contract between source-specific world stores and the renderer's
 * temporary L2 64x64 column. Source-side storage (dense terrain pages,
 * sparse object octrees) stays untouched by this contract.
 */

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { ChunkVolume } from "./volume.ts";
import type { ColumnRunWriter } from "./voxel_scan.ts";

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

  /**
   * The direct path for coarse tiers: the layer's content at `key` as one
   * list of runs per column, read straight from the layer's own source
   * without a volume or a scan. Returns false when the layer has nothing at
   * `key`.
   *
   * This is how every established terrain LOD builds its coarse levels —
   * from the source at the level's own sampling, never by folding the fine
   * level (issue/20260912-coarse-tiers-what-the-field-does.md). The
   * terrain page is a heightmap, so its runs are one per column and cost a
   * few array reads; painting the same heightmap into a 64³ volume and
   * scanning it back into runs cost ~3 ms per chunk for the same result.
   *
   * Optional. A layer that does not offer it forces the scan path for any
   * key where it has content, so nothing is silently dropped: a layer whose
   * content cannot be expressed as plain column runs (carved terrain,
   * sub-cell objects) simply does not implement this.
   */
  runs?(key: ChunkKey, w: ColumnRunWriter): boolean;

  /**
   * Whether this layer has any content at `key`, for a layer that does not
   * implement `runs`: the node takes the direct path only when every layer
   * either offers runs or is empty here. Optional; absent means "assume
   * content", which keeps the scan path.
   */
  present?(key: ChunkKey): boolean;
}

/** Registration order is override priority: later layers win. */
export type WorldLayerStack = readonly WorldLayer[];
