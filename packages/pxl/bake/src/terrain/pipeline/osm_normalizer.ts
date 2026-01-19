// @pxl/bake/terrain/pipeline/osm_normalizer.ts
// OSM feature 객체 생성 로직 (정책 적용 및 좌표 정규화)

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";
import { Arena } from "@xpute/spec/lib/ds/arena.ts";

import type { V3 } from "@pxl/spec/space/v3.ts";
import type { G3857 } from "@pxl/spec/space/adapter/3857.ts";
import { XY_CELL_PER_M } from "@pxl/bake/terrain/constants.ts";

import { type OSMExtra, type OSMFeature, type OSMRawFeature, OSMType } from "../model/osm_feature.ts";
import { compute_extra, compute_metrics, determine_category, should_force_closed } from "./osm_policy.ts";

export const V3_ARENA = new Arena<V3>(() => [0, 0, 0], { init_cap: 1 << 12 });

const project_to_local_cell = (dst: V3, src: G3857, page_min: G3857): void => {
  dst[0] = (src[0] - page_min[0]) * XY_CELL_PER_M;
  dst[1] = (src[1] - page_min[1]) * XY_CELL_PER_M;
  dst[2] = 0; // 2D 지형 래스터라이징이므로 Z는 0
};

/** Standard input (3857) -> Final OSM feature */
export const normalize = (raw_feat: OSMRawFeature, page_min: G3857, page_z_scale: f64): OSMFeature => {
  const { id, type, tags, geometry, center, is_polygon_source } = raw_feat;
  const { points, holes = [] } = geometry;

  // 1. Validation: 점 개수 부족 방어 로직 (Node 제외)
  if (type !== "node" && points.length < 2) {
    throw new Error(`Invalid geometry for feature ${id}: insufficient points`);
  }

  // 2. Policy Check: 태그 기반 카테고리 결정
  const category = determine_category(tags);

  // 3. Closed Loop Detection: 폐곡 여부 판단
  // - 시작점과 끝점의 일치 여부 (오차범위 0.1m)
  const is_geometrically_closed = points.length >= 3 &&
    Math.abs(points[0][0] - points[points.length - 1][0]) < 0.1 &&
    Math.abs(points[0][1] - points[points.length - 1][1]) < 0.1;

  // - 도로(highway)와 수로(waterway)는 닫혀있어도 선형으로 취급하는 원칙 보존
  const is_closed = should_force_closed(tags) ||
    (!tags.highway && !tags.waterway &&
      (is_polygon_source || is_geometrically_closed));

  // 4. Metric Computation: 물리적 수치 계산 (높이, 층수 등)
  const raw_computed = compute_metrics(tags, category);

  // 5. Extra Flags: 별도 객체로 분리하여 계산
  const extra: OSMExtra = compute_extra(tags, category);

  // 6. M -> Cell 스케일링

  const computed: OSMFeature["computed"] = {
    height: raw_computed.height * XY_CELL_PER_M * page_z_scale,
    width: raw_computed.width * XY_CELL_PER_M,
    layer: raw_computed.layer,
  };

  // 7. Center Correction: 중심점 보정
  const final_center_3857: G3857 = center || points[0] || [0, 0, 0];

  // 8. Geometry Processing with V3 Arena
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

  // 9. Final Construction
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
