import * as THREE from 'three';
import type {
  OverpassElement,
  OverpassResponse,
} from '@/entities/osm/model/types';
import type { Location, MeterPos } from '@/shared/types/map';

/**
 * 지도의 물리적 기반 시설 (3D Mesh 렌더링용)
 */
export interface MapInfra {
  id: string;
  // 인프라의 세부 유형 (물, 땅의 용도 구획 추가)
  type: 'building' | 'road' | 'water' | 'landuse';
  points: THREE.Vector3[]; // 상대 좌표
  center: THREE.Vector3; // 절대 좌표 기준점
  shape?: THREE.Shape; // 건물을 위한 외곽선
  height: number; // 건물일 경우의 높이
  tags: Record<string, string | undefined>; // 원본 태그 (추후 확장성)
}

/**
 * 지도의 정보성 지점 (HTML/Badge 라벨용)
 */
export interface MapPoi {
  id: string;
  type: 'poi';
  category: string; // cafe, restaurant 등
  name: string;
  center: THREE.Vector3; // 라벨이 표시될 절대 좌표
  tags: Record<string, string | undefined>;
}

// 최종 반환 구조
export interface ParseResult {
  infra: MapInfra[];
  pois: MapPoi[];
}
export const parseOsmData = (
  data: OverpassResponse,
  project: (loc: Location) => MeterPos | null,
): ParseResult => {
  const result: ParseResult = { infra: [], pois: [] };
  if (!data?.elements) return result;

  data.elements.forEach((item: OverpassElement, idx: number) => {
    const tags = item.tags || {};

    // --- [Part 1] POI 데이터 추출 (이름이 있는 모든 요소 대상) ---
    const rawLat = item.lat || item.center?.lat;
    const rawLng = item.lon || item.center?.lon;

    if (tags.name && rawLat && rawLng) {
      const poiPos = project({ lng: rawLng, lat: rawLat, alt: 0 });
      if (poiPos) {
        result.pois.push({
          id: `poi-${item.id || idx}`,
          type: 'poi',
          category: tags.amenity || tags.shop || tags.tourism || 'place',
          name: tags.name,
          center: new THREE.Vector3(poiPos.x, 0, poiPos.z),
          tags,
        });
      }
    }

    // --- [Part 2] Infrastructure 데이터 추출 (물리적 형체 위주) ---
    if (item.type === 'way' && item.geometry) {
      const isBuilding = !!tags.building;
      const isRoad = !!tags.highway;
      const isWater = !!(
        tags.natural === 'water' ||
        tags.waterway ||
        tags.water
      );
      const isLanduse = !!tags.landuse;

      // 물리적 실체가 없는 데이터는 스킵
      if (!isBuilding && !isRoad && !isWater && !isLanduse) return;

      const absolutePoints = item.geometry
        .map((pt) => project({ lng: pt.lon, lat: pt.lat, alt: 0 }))
        .filter((v): v is MeterPos => v !== null);

      if (absolutePoints.length < 2) return;

      // 기준점 설정
      const first = absolutePoints[0];
      const center = new THREE.Vector3(first.x, 0, first.z);

      // 타입별 높이 오프셋 (Z-fighting 방지)
      const points = absolutePoints.map((pt) => {
        let y = 0;
        if (isRoad) y = 0.05; // 도로는 지면보다 위
        if (isWater) y = -0.1; // 물은 지면보다 아래
        if (isLanduse) y = 0.02; // 구획(공원 등)은 도로보다 아래
        return new THREE.Vector3(pt.x - center.x, y, pt.z - center.z);
      });

      // --- [수정된 Shape 생성 로직] ---
      let shape: THREE.Shape | undefined;

      // 1. 도로(isRoad)를 조건에 추가했습니다.
      // 2. Three.js의 Shape는 2D(x, y) 좌표계입니다.
      //    우리의 지도는 바닥면이 (x, z)이므로, z값을 Shape의 y축으로 매핑해야 합니다.
      if (
        (isBuilding || isWater || isLanduse || isRoad) &&
        points.length >= 2
      ) {
        shape = new THREE.Shape();

        // Shape의 2D x, y를 (points.x, points.z)로 매핑
        shape.moveTo(points[0].x, points[0].z);
        for (let i = 1; i < points.length; i++) {
          shape.lineTo(points[i].x, points[i].z);
        }

        // 도로는 선(Line)인 경우가 많으므로 닫지 않고,
        // 건물, 물, 땅은 면(Area)이므로 닫아줍니다.
        if (!isRoad && points.length >= 3) {
          shape.closePath();
        }
      }

      // 건물 높이 계산
      const height = tags.height
        ? parseFloat(tags.height)
        : tags['building:levels']
          ? parseFloat(tags['building:levels']) * 3.5
          : 10;

      result.infra.push({
        id: `infra-${item.id || idx}`,
        type: isBuilding
          ? 'building'
          : isRoad
            ? 'road'
            : isWater
              ? 'water'
              : 'landuse',
        points,
        center,
        shape,
        height: isBuilding ? height : 0,
        tags,
      });
    }
  });

  return result;
};
