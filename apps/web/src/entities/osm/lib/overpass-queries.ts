/**
 * Overpass QL 최적화 설명:
 * 1. [out:json]: JSON 형식으로 출력
 * 2. way(around...): 중심점 기준 반경 내 '도로(highway)' 검색
 * 3. out geom: [중요] 각 way의 노드 좌표(lat, lon)를 geometry 배열로 포함하여 출력.
 * 이 옵션이 있어야 별도의 노드 조인 없이 바로 선을 그릴 수 있습니다.
 */

// 1. 프로젝트 공통 타입 (Shared)
import type { Location } from '@/shared/types/map';

/**
 * Overpass QL 쿼리 파라미터 타입
 */
type MapQueriesParams = {
  location: Location;
  radius: number;
};

/**
 * Overpass QL 쿼리 모음집
 * [out:json]: JSON 형식 출력
 * [timeout:25]: 25초 후 타임아웃
 * out body geom: 요소의 속성(tags)과 기하학적 좌표(geometry)를 한 번에 포함
 */
export const MAP_QUERIES = {
  /**
   * 1. [기존] 전체 지형지물 렌더링용 쿼리 (Heavy)
   * 도로와 모든 건물을 가져옵니다. (기존 로직 유지)
   */
  GEOMETRY_ALL: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    return `
    [out:json][timeout:25];
    (way["highway"](around:${radius},${lat},${lng});
    way["building"](around:${radius},${lat},${lng});
    relation["building"](around:${radius},${lat},${lng}););
    out body geom;
    >;
    out skel qt;
    `;
  },

  /**
   * 2. [추가] 도로 전용 쿼리 (Ultra Light)
   * 건물 데이터를 완전히 배제하고 도로망만 가져옵니다.
   * 성능이 가장 좋으며 렌더링 부하가 거의 없습니다.
   */
  GEOMETRY_ROADS_ONLY: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    return `
    [out:json][timeout:25];
    (way["highway"](around:${radius},${lat},${lng}););
    out body geom;
    >;
    out skel qt;
    `;
  },

  /**
   * 3. [추가] 절충안 쿼리 (Balanced)
   * 모든 도로는 가져오되, 건물은 '주요 건물'만 필터링해서 가져옵니다.
   * 주택(house, apartments)을 제외한 상업, 공공, 랜드마크 위주로 구성됩니다.
   */
  GEOMETRY_MAJOR_ONLY: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    // 필터링할 건물 태그: 상업, 소매, 공공, 병원, 대학, 종교 등
    const buildingFilter =
      '"building"~"commercial|retail|public|civic|hospital|university|church|cathedral|government"';
    return `
    [out:json][timeout:25];
    (
      way["highway"](around:${radius},${lat},${lng});
      way[${buildingFilter}](around:${radius},${lat},${lng});
      relation[${buildingFilter}](around:${radius},${lat},${lng});
    );
    out body geom;
    >;
    out skel qt;
    `;
  },

  /**
   * 편의시설 마커 표시용 쿼리
   */
  AMENITIES: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    return `
    [out:json][timeout:25];
    node["amenity"](around:${radius},${lat},${lng});
    out body geom;
    `;
  },

  /**
   * 특정 카테고리(카페) 필터링 쿼리
   */
  CAFE: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    return `
    [out:json][timeout:25];
    node["amenity"="cafe"](around:${radius},${lat},${lng});
    out body geom;
    `;
  },
} as const;
