/**
 * @file overpass-boundary-queries.ts
 * 행정 구역 및 논리적 경계(Boundary) 수집을 위한 Overpass QL 쿼리 셋입니다.
 */

/**
 * OSM 태그 그룹 분리 (Reusable Filter Sets)
 */
export const BOUNDARY_FILTERS = {
  /** 행정 구역 (시/군/구/동 경계)
   * admin_level 4(도/시), 5~8(구/군/동/면) 레벨을 커버합니다.
   */
  ADMIN: (bbox: string) => `
    relation["boundary"="administrative"]["admin_level"~"4|5|6|7|8"](${bbox});
  `,

  /** 주거 단지 및 주요 시설 구역
   * 아파트 단지, 대학 캠퍼스, 병원 단지 등 논리적인 덩어리를 표현합니다.
   */
  RESIDENTIAL: (bbox: string) => `
    nwr["landuse"="residential"](${bbox});
    nwr["amenity"~"university|hospital|school|college"](${bbox});
    nwr["campus"](${bbox});
  `,

  /** 공원 및 녹지 영역 (논리적 구역)
   * 지형(Geometry)과 별개로 '공원 구역'이라는 명칭과 범위를 식별하기 위함입니다.
   */
  LEISURE: (bbox: string) => `
    nwr["leisure"~"park|nature_reserve|recreation_ground"](${bbox});
  `,
} as const;

/**
 * tile-processor 연동용 쿼리 객체
 */
export const BOUNDARY_QUERIES = {
  /**
   * [GET_RAW_FILTER]
   * 모든 종류의 경계(행정, 주거, 여가) 데이터를 하나로 합성하여 반환합니다.
   */
  GET_RAW_FILTER: (bbox: string) => `
    ${BOUNDARY_FILTERS.ADMIN(bbox)}
    ${BOUNDARY_FILTERS.RESIDENTIAL(bbox)}
    ${BOUNDARY_FILTERS.LEISURE(bbox)}
  `,
} as const;
