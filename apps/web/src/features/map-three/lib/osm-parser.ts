import * as THREE from 'three';
import type {
  OverpassElement,
  OverpassResponse,
} from '@/entities/osm/model/types';
import type { Location, MeterPos } from '@/shared/types/map';

export interface ParsedFeature {
  id: string;
  type: 'road' | 'building';
  points: THREE.Vector3[];
  center: THREE.Vector3; // [추가] 객체의 기준점이 될 절대 미터 좌표
  shape?: THREE.Shape; // 건물 외곽선 Shape (ExtrudeGeometry용)
  height: number; // 건물 높이(m)
  tags?: Record<string, string | undefined>;
}

export const parseOsmData = (
  data: OverpassResponse,
  project: (loc: Location) => MeterPos | null, // Location 객체를 받는 project 함수
): ParsedFeature[] => {
  if (!data?.elements) return [];

  return data.elements.reduce<ParsedFeature[]>(
    (acc, item: OverpassElement, idx: number) => {
      if (item.type !== 'way' || !item.geometry) return acc;

      const isBuilding = !!item.tags?.building;

      /**
       * 1. 좌표 변환 (Location -> MeterPos)
       * OSM 데이터의 lon, lat을 객체로 감싸서 전달합니다.
       */
      const absolutePoints = item.geometry
        .map((pt) => {
          // OSM geometry 포인트에는 고도 정보가 없는 경우가 많으므로 기본값 0 사용
          const meterPos = project({ lng: pt.lon, lat: pt.lat, alt: 0 });
          return meterPos;
        })
        .filter((v): v is MeterPos => v !== null);

      if (absolutePoints.length < 2) return acc;

      /**
       * [보강] 기준점(Center) 및 상대 좌표 계산
       * Three.js의 정밀도 문제를 방지하기 위해 첫 번째 점을 center(절대좌표)로 잡고
       * 나머지 점들은 center 기준의 상대 좌표로 변환합니다.
       */
      const first = absolutePoints[0];
      const center = new THREE.Vector3(first.x, 0, first.z);

      const points = absolutePoints.map((pt) => {
        // 도로는 Z-fighting 방지를 위해 지면보다 살짝(0.05m) 띄웁니다.
        const yOffset = isBuilding ? 0 : 0.05;
        return new THREE.Vector3(
          pt.x - center.x, // 절대 좌표에서 기준점 차감 (상대 좌표화)
          0 + yOffset,
          pt.z - center.z, // 절대 좌표에서 기준점 차감 (상대 좌표화)
        );
      });

      /**
       * 2. 건물을 위한 2D Shape 생성
       * ExtrudeGeometry를 위해 XZ 평면의 좌표를 Three.js Shape의 XY 평면으로 매핑합니다.
       * 이제 상대 좌표를 사용하므로 0,0 근처에서 Shape이 생성되어 정밀도가 유지됩니다.
       */
      let shape: THREE.Shape | undefined;
      if (isBuilding && points.length >= 3) {
        shape = new THREE.Shape();
        shape.moveTo(points[0].x, points[0].z);
        for (let i = 1; i < points.length; i++) {
          shape.lineTo(points[i].x, points[i].z);
        }
        shape.closePath();
      }

      /**
       * 3. 실제 미터 단위 높이 결정
       * 1 unit = 1 meter 시스템이므로 파싱된 높이값을 그대로 사용합니다.
       * - height 태그가 있으면 사용
       * - 층수(levels) 태그가 있으면 층당 3.5m로 계산
       * - 기본값 10m
       */
      const height = item.tags?.height
        ? parseFloat(item.tags.height)
        : item.tags?.['building:levels']
          ? parseFloat(item.tags['building:levels']) * 3.5
          : 10;

      acc.push({
        id: `osm-${item.id || idx}`,
        type: isBuilding ? 'building' : 'road',
        points,
        center, // 절대 좌표 기준점 보관
        shape,
        height,
        tags: item.tags,
      });

      return acc;
    },
    [],
  );
};
