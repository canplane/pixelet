/**
 * @file osm-poi-queries.ts
 * 지도 상의 주요 POI(관심 지점) 수집을 위한 Overpass QL 쿼리 셋입니다.
 */

/**
 * OSM 태그 그룹 분리 (Reusable Filter Sets)
 */
export const POI_FILTERS = {
  /** * [Pass 1] Major POI (거점, 랜드마크)
   * 길 찾기의 기준이 되는 핵심 교통 허브 및 공공 기관입니다.
   */
  MAJOR: (bbox: string) => `
    node["railway"~"station|subway_entrance"](${bbox});
    node["amenity"~"townhall|hospital|university|police|fire_station"](${bbox});
    node["tourism"~"museum|viewpoint|attraction|gallery"](${bbox});
    node["historic"~"memorial|monument|castle"](${bbox});
  `,
  /** * [Pass 2] Sub POI (생활 편의)
   * 카페, 음식점, 쇼핑몰 등 일상적인 상점 데이터입니다.
   */
  SUB: (bbox: string) => `
    node["amenity"~"cafe|restaurant|pub|bar|bakery"](${bbox});
    node["shop"~"mall|department_store|supermarket|convenience"](${bbox});
    node["amenity"~"arts_centre|theatre|cinema|library"](${bbox});
  `,
} as const;

/**
 * tile-processor 연동용 쿼리 객체
 */
export const POI_QUERIES = {
  /**
   * [GET_RAW_FILTER]
   * tile-processor에서 POI 레이어를 요청할 때 사용하는 단일 진입점입니다.
   * Major(거점)와 Sub(상점) 데이터를 모두 합쳐서 반환합니다.
   */
  GET_RAW_FILTER: (bbox: string) => `
    ${POI_FILTERS.MAJOR(bbox)}
    ${POI_FILTERS.SUB(bbox)}
  `,
} as const;
