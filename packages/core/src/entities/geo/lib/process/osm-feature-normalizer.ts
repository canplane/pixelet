/**
 * @file osm-feature-normalizer.ts
 * @description 엔진 표준 OsmFeature 객체 생성을 위한 순수 도메인 로직 (정책 적용)
 */
import type { OsmFeature } from '../../model/osm-feature';
import {
  determineOsmCategory,
  shouldForceClosed,
  computeOsmMetrics,
  computeOsmExtra,
} from '../../policy/osm-policy';
import type { OsmInput } from '@pixelet/core/entities/geo/model/osm-feature';

/**
 * [Core Logic] normalizeOsmFeature
 * Standard Input -> Final OsmFeature (Domain Policy Applied)
 */
export const normalizeOsmFeature = (input: OsmInput): OsmFeature => {
  const { id, type, tags, geometry, center, isPolygonSource } = input;
  const { points, holes = [] } = geometry;

  // 1. [Validation] 점 개수 부족 방어 로직 (Node 제외)
  if (type !== 'node' && points.length < 2) {
    throw new Error(`Invalid geometry for feature ${id}: insufficient points`);
  }

  // 2. [Policy Check] 태그 기반 카테고리 결정 (building, road, water 등)
  const category = determineOsmCategory(tags);

  // 3. [Closed Loop Detection] 폐곡 여부 판단
  // - 시작점과 끝점의 일치 여부 (오차범위 0.1m)
  const isGeometricallyClosed =
    points.length >= 3 &&
    Math.abs(points[0].x - points[points.length - 1].x) < 0.1 &&
    Math.abs(points[0].y - points[points.length - 1].y) < 0.1;

  // - 도로(highway)와 수로(waterway)는 닫혀있어도 선형으로 취급하는 원칙 보존
  const isClosed =
    shouldForceClosed(tags) ||
    (!tags.highway &&
      !tags.waterway &&
      (isPolygonSource || isGeometricallyClosed));

  // 4. [Metric Computation] 물리적 수치 계산 (높이, 층수 등)
  const computed = computeOsmMetrics(tags, category);

  // 🚀 5. [Extra Flags] 별도 객체로 분리하여 계산
  const extra = computeOsmExtra(tags, category);

  // 6. [Center Correction] 중심점 보정
  const finalCenter = center || points[0] || { x: 0, y: 0, z: 0 };

  return {
    type: points.length === 1 ? 'node' : 'way',
    id,
    center: finalCenter,
    shape: {
      points,
      isClosed,
      holes: holes.length > 0 ? holes : undefined,
    },
    computed,
    tags,
    category,
    extra,
  };
};
