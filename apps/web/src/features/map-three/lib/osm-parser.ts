import * as THREE from 'three';
import type {
  OverpassElement,
  OverpassResponse,
} from '@/entities/osm/model/types';

export interface ParsedFeature {
  id: string;
  type: 'road' | 'building';
  points: THREE.Vector3[];
  shape?: THREE.Shape; // 건물 외곽선 Shape 추가
  height: number; // 건물 높이(m)
  tags?: Record<string, string | undefined>;
}

export const parseOsmData = (
  data: OverpassResponse,
  project: (lngLat: [number, number]) => [number, number, number] | null,
): ParsedFeature[] => {
  if (!data?.elements) return [];

  return data.elements.reduce<ParsedFeature[]>(
    (acc, item: OverpassElement, idx: number) => {
      if (item.type !== 'way' || !item.geometry) return acc;

      const isBuilding = !!item.tags?.building;

      // 1. 좌표 변환
      const points = item.geometry
        .map((pt) => {
          const relPos = project([pt.lon, pt.lat]);
          // 건물은 바닥(y=0)에서 시작해서 위로 뽑을 것이므로 yOffset을 0으로 둡니다.
          const yOffset = isBuilding ? 0 : 0.05;
          return relPos
            ? new THREE.Vector3(relPos[0], yOffset, relPos[2])
            : null;
        })
        .filter((v): v is THREE.Vector3 => v !== null);

      if (points.length < 2) return acc;

      // 2. 건물을 위한 Shape 생성 (x, z 평면 좌표 활용)
      let shape: THREE.Shape | undefined;
      if (isBuilding && points.length >= 3) {
        shape = new THREE.Shape();
        // Three.js Shape는 2D(x, y) 평면이므로, 우리의 (x, z)를 (x, y)로 매핑합니다.
        shape.moveTo(points[0].x, points[0].z);
        for (let i = 1; i < points.length; i++) {
          shape.lineTo(points[i].x, points[i].z);
        }
        shape.closePath();
      }

      // 3. 건물 높이 결정 (데이터에 없으면 기본값 10m)
      const height = item.tags?.height
        ? parseFloat(item.tags.height)
        : item.tags?.['building:levels']
          ? parseFloat(item.tags['building:levels']) * 3.5
          : 10;

      acc.push({
        id: `osm-${item.id || idx}`,
        type: isBuilding ? 'building' : 'road',
        points,
        shape,
        height,
        tags: item.tags,
      });

      return acc;
    },
    [],
  );
};
