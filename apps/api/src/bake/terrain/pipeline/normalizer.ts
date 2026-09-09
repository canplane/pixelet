// @pxl/api/bake/terrain/pipeline/normalizer.ts
// Raw feature normalization and local coordinate projection — provider-agnostic,
// parameterized by a NormalizePolicy so coast_policy.ts (coastline) and
// overture_policy.ts (Buildings/Transportation/Base) share the same geometry
// projection and closure-detection code instead of duplicating it.

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { Arena } from "@xpute/core/ds/arena.ts";

import { fill3, type V3 } from "@pxl/shared/spec/space/v3.ts";
import type { G3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import { XY_CELL_PER_M } from "@pxl/shared/bake/constants.ts";
import type { CellConfig } from "@pxl/shared/bake/format.ts";
import type { Feature, FeatureExtra } from "@pxl/shared/bake/model/feature.ts";

export const V3_ARENA = new Arena<V3>(() => fill3(0), { init_cap: 1 << 12 });

/** A raw feature shape any policy can normalize — CoastRawFeature and each Overture*RawFeature all satisfy this. */
export type RawFeature = {
  geometry: {
    points: G3857[];
    holes?: G3857[][];
    is_polygon_source?: boolean;
  };
  center?: G3857;
};

export type NormalizePolicy<F extends RawFeature> = {
  // a provider-neutral id — CoastRawFeature has `id`, Overture*RawFeature has `gers_id`.
  get_id: (raw: F) => string;
  determine_category: (raw: F) => string;
  compute_metrics: (raw: F, category: string) => Feature["computed"];
  compute_extra: (raw: F, category: string) => FeatureExtra;
  determine_cell_cfg: (raw: F, category: string, extra: FeatureExtra) => CellConfig | null;
  should_force_closed: (raw: F) => boolean;
  /** True for features that must stay linear (never fill-closed) regardless of geometry closure — coastline, Overture segments. */
  is_linear: (raw: F) => boolean;
};

/** Projects a 3857 point into page-local cell coordinates. */
const project_to_local_cell = (dst: V3, src: G3857, page_min: G3857): void => {
  dst[0] = (src[0] - page_min[0]) * XY_CELL_PER_M;
  dst[1] = (src[1] - page_min[1]) * XY_CELL_PER_M;
  dst[2] = 0;
};

/** Converts a raw 3857-space feature into the final local normalized feature. */
export const normalize = <F extends RawFeature>(
  raw_feat: F,
  page_min: G3857,
  page_z_scale: f64,
  policy: NormalizePolicy<F>,
): Feature => {
  const { geometry, center } = raw_feat;
  const { points, holes = [], is_polygon_source } = geometry;
  const id = policy.get_id(raw_feat);

  // 1. Validation
  if (points.length === 0) {
    throw new Error(`Invalid geometry for feature ${id}: no points`);
  }

  // 2. Category policy
  const category = policy.determine_category(raw_feat);

  // 3. Closed-loop detection
  // - geometric closure based on first/last point match within 0.1m
  const is_geometrically_closed = points.length >= 3 &&
    Math.abs(points[0][0] - points[points.length - 1][0]) < 0.1 &&
    Math.abs(points[0][1] - points[points.length - 1][1]) < 0.1;

  const is_closed = !policy.is_linear(raw_feat) &&
    (policy.should_force_closed(raw_feat) || is_polygon_source || is_geometrically_closed);

  // 4. Metric computation
  const metrics_m: Feature["computed"] = policy.compute_metrics(raw_feat, category);

  // 5. Extra flags
  const extra: FeatureExtra = policy.compute_extra(raw_feat, category);

  // 6. Cell config (target ground channel + cell type, or null to skip rasterization)
  const cell_cfg: CellConfig | null = policy.determine_cell_cfg(raw_feat, category, extra);

  // 7. Meter -> cell scaling
  const computed: Feature["computed"] = {
    height: metrics_m.height * XY_CELL_PER_M * page_z_scale,
    width: metrics_m.width * XY_CELL_PER_M,
    layer: metrics_m.layer,
  };

  // 8. Center resolution
  const final_center_3857: G3857 = center || points[0] || [0, 0, 0];

  // 9. Geometry projection with V3 arena
  // 9-a. Center
  const local_center: V3 = V3_ARENA.alloc();
  project_to_local_cell(local_center, final_center_3857, page_min);

  // 9-b. Outer Points
  const local_points: V3[] = new Array(points.length);
  for (let i: u32 = 0; i < points.length; i++) {
    const p: V3 = V3_ARENA.alloc();
    project_to_local_cell(p, points[i], page_min);
    local_points[i] = p;
  }

  // 9-c. Holes
  let local_holes: V3[][] | undefined;
  if (holes.length > 0) {
    local_holes = new Array(holes.length);
    for (let i: u32 = 0; i < holes.length; i++) {
      const hole_ring = holes[i];
      const local_ring = new Array(hole_ring.length);
      for (let j: u32 = 0; j < hole_ring.length; j++) {
        const p: V3 = V3_ARENA.alloc();
        project_to_local_cell(p, hole_ring[j], page_min);
        local_ring[j] = p;
      }
      local_holes[i] = local_ring;
    }
  }

  // 10. Final construction
  return {
    id,
    center: local_center,
    shape: { points: local_points, is_closed, holes: local_holes },
    computed,
    category,
    extra,
    cell_cfg,
  };
};
