/**
 * @file osm-normalizer.ts
 * Raw OSM(4326) 데이터를 엔진 표준(3857 Meter)으로 정규화합니다.
 */

import type { OverpassElement } from '../api/overpass-types';
import type { NormalizedElement } from '../api/normalized-types';
import { projection } from '@pixelet/shared/shared/lib/map/projection';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';

// 🎯 [Refactoring] 공유 정책 모듈 Import
import {
  determineOsmCategory,
  shouldForceClosed,
  computeOsmMetrics,
} from './osm-policy';

const { project } = projection;

export const normalizeOsmData = (
  elements: OverpassElement[],
): NormalizedElement[] => {
  if (!elements || elements.length === 0) return [];

  const nodeCache = new Map<number, MeterPos>();
  elements.forEach((el) => {
    if (el.type === 'node' && el.lat !== undefined && el.lon !== undefined) {
      const projected = project({ lat: el.lat, lng: el.lon, alt: 0 });
      if (projected) nodeCache.set(el.id, projected);
    }
  });

  return elements
    .map((item, idx): NormalizedElement | null => {
      const tags = item.tags || {};

      // 1. 위치 정보 정규화
      const rawLat = item.lat || item.center?.lat;
      const rawLon = item.lon || item.center?.lon;
      const center =
        rawLat !== undefined && rawLon !== undefined
          ? project({ lat: rawLat, lng: rawLon, alt: 0 })
          : undefined;

      // 2. 좌표 추출
      const resolvePoints = (geom?: any[], nodes?: number[]): MeterPos[] => {
        if (geom && geom.length > 0) {
          return geom
            .map((pt) => project({ lng: pt.lon, lat: pt.lat, alt: 0 }))
            .filter((v): v is MeterPos => v !== null);
        }
        if (nodes && nodes.length > 0) {
          return nodes
            .map((id) => nodeCache.get(id))
            .filter((v): v is MeterPos => v !== undefined) as MeterPos[];
        }
        return [];
      };

      let points: MeterPos[] = [];
      const holes: MeterPos[][] = [];

      if (item.type === 'way') {
        points = resolvePoints(item.geometry, item.nodes);
      } else if (item.type === 'relation' && item.members) {
        item.members.forEach((member) => {
          const mPoints = resolvePoints(member.geometry, member.nodes);
          if (mPoints.length < 2) return;
          if (member.role === 'inner') holes.push(mPoints);
          else points.push(...mPoints);
        });
      }

      if (item.type !== 'node' && points.length < 2) return null;

      // 🎯 [Refactoring] 분류 로직 위임 (Policy 모듈 사용)
      const category = determineOsmCategory(tags);

      // 🎯 [Refactoring] 강제 폐곡 판정 위임 + 기하학적 검사 결합
      const isClosed =
        item.type === 'relation' ||
        shouldForceClosed(tags) ||
        (!tags.highway && !tags.waterway && points.length >= 3) ||
        (points.length > 2 &&
          Math.abs(points[0].x - points[points.length - 1].x) < 0.1);

      // 🎯 [Refactoring] 물리 수치 계산 위임 (Policy 모듈 사용)
      const computed = computeOsmMetrics(tags, category);

      return {
        type: item.type,
        id: item.id || idx,
        center: center || points[0],
        shape: {
          points,
          isClosed,
          holes: holes.length > 0 ? holes : undefined,
        },
        computed, // 계산된 객체 그대로 주입
        tags,
        category,
      };
    })
    .filter((v): v is NormalizedElement => v !== null);
};
