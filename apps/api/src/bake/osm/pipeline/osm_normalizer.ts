// @pxl/api/bake/osm/pipeline/osm_normalizer.ts
// OSM feature normalization and local coordinate projection.

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { Arena } from "@xpute/core/ds/arena.ts";

import { fill3, type V3 } from "@pxl/shared/spec/space/v3.ts";
import type { G3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import { XY_CELL_PER_M } from "@pxl/shared/bake/constants.ts";

import { type OSMExtra, type OSMFeature, type OSMRawFeature, OSMType } from "@pxl/shared/bake/model/osm_feature.ts";
// osm_policy.ts stays under bake/terrain/ — it's genuinely mixed (POI
// category rules alongside terrain-cell config), not cleanly splittable
// without untangling that, unlike this file, poi_extractor.ts, and
// osm_adapter.ts, which had zero terrain-specific content to begin with.
import { compute_extra, compute_metrics, determine_category, should_force_closed } from "@pxl/api/bake/terrain/pipeline/osm_policy.ts";

export const V3_ARENA = new Arena<V3>(() => fill3(0), { init_cap: 1 << 12 });

/** Projects a 3857 point into page-local cell coordinates. */
const project_to_local_cell = (dst: V3, src: G3857, page_min: G3857): void => {
  dst[0] = (src[0] - page_min[0]) * XY_CELL_PER_M;
  dst[1] = (src[1] - page_min[1]) * XY_CELL_PER_M;
  dst[2] = 0;
};

/** Converts a raw 3857-space OSM feature into the final local normalized feature. */
export const normalize = (raw_feat: OSMRawFeature, page_min: G3857, page_z_scale: f64): OSMFeature => {
  const { id, type, tags, geometry, center, is_polygon_source } = raw_feat;
  const { points, holes = [] } = geometry;

  // 1. Validation
  if (type !== "node" && points.length < 2) {
    throw new Error(`Invalid geometry for feature ${id}: insufficient points`);
  }

  // 2. Category policy
  const category = determine_category(tags);

  // 3. Closed-loop detection
  // - geometric closure based on first/last point match within 0.1m
  const is_geometrically_closed = points.length >= 3 &&
    Math.abs(points[0][0] - points[points.length - 1][0]) < 0.1 &&
    Math.abs(points[0][1] - points[points.length - 1][1]) < 0.1;

  // - roads and waterways remain linear even if geometrically closed
  const is_closed = should_force_closed(tags) ||
    (!tags.highway && !tags.waterway &&
      (is_polygon_source || is_geometrically_closed));

  // 4. Metric computation
  const metrics_m: OSMFeature["computed"] = compute_metrics(tags, category);

  // 5. Extra flags
  const extra: OSMExtra = compute_extra(tags, category);

  // 6. Meter -> cell scaling
  const computed: OSMFeature["computed"] = {
    height: metrics_m.height * XY_CELL_PER_M * page_z_scale,
    width: metrics_m.width * XY_CELL_PER_M,
    layer: metrics_m.layer,
  };

  // 7. Center resolution
  const final_center_3857: G3857 = center || points[0] || [0, 0, 0];

  // 8. Geometry projection with V3 arena
  // 8-a. Center
  const local_center: V3 = V3_ARENA.alloc();
  project_to_local_cell(local_center, final_center_3857, page_min);

  // 8-b. Outer Points
  const local_points: V3[] = new Array(points.length);
  for (let i: u32 = 0; i < points.length; i++) {
    const p: V3 = V3_ARENA.alloc();
    project_to_local_cell(p, points[i], page_min);
    local_points[i] = p;
  }

  // 8-c. Holes
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

  // 9. Final construction
  return {
    type: points.length === 1 ? OSMType.NODE : OSMType.WAY,
    id,
    center: local_center,
    shape: { points: local_points, is_closed, holes: local_holes },
    computed,
    tags,
    category,
    extra,
  };
};
