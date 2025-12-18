/**
 * Overpass QL 최적화 설명:
 * 1. [out:json]: JSON 형식으로 출력
 * 2. way(around...): 중심점 기준 반경 내 '도로(highway)' 검색
 * 3. out geom: [중요] 각 way의 노드 좌표(lat, lon)를 geometry 배열로 포함하여 출력.
 * 이 옵션이 있어야 별도의 노드 조인 없이 바로 선을 그릴 수 있습니다.
 */

import type { LngLat } from '@/shared/types/map';

type MapQueriesParams = {
  lngLat: LngLat;
  radius: number;
};

export const MAP_QUERIES = {
  // 지형지물 (3D 렌더링용)
  GEOMETRY: ({ lngLat, radius }: MapQueriesParams) => {
    const [lng, lat] = lngLat;
    return `
      [out:json][timeout:25];
      (
        way["highway"](around:${radius}, ${lat}, ${lng});
        way["building"](around:${radius}, ${lat}, ${lng});
        relation["building"](around:${radius}, ${lat}, ${lng});
      );
      out body geom;
      >;
      out skel qt;
    `;
  },

  // 편의시설 (마커/텍스트 리스트용)
  AMENITIES: ({ lngLat, radius }: MapQueriesParams) => {
    const [lng, lat] = lngLat;
    return `
      [out:json][timeout:25];
      node["amenity"](around:${radius}, ${lat}, ${lng});
      out body geom;
    `;
  },

  // 특정 카테고리 (예: 카페만)
  CAFE: ({ lngLat, radius }: MapQueriesParams) => {
    const [lng, lat] = lngLat;
    return `
      [out:json][timeout:25];
      node["amenity"="cafe"](around:${radius}, ${lat}, ${lng});
      out body geom;
    `;
  },
};
