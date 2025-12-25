/**
 * @file osm-geometry-queries.ts
 * 복셀 지형 및 물리적 구조물 생성을 위한 Overpass QL 쿼리 셋입니다.
 */

/**
 * OSM 태그 그룹 분리 (Reusable Filter Sets)
 */
export const GEOMETRY_FILTERS = {
  /** * [Pass 1] base-layer (지형 및 토지 용도)
   * 캔버스의 가장 밑바닥(Background)을 결정합니다.
   */
  BASE: (bbox: string) => `
    nwr["landuse"~"forest|grass|residential|commercial|industrial|cemetery|allotments|quarry|construction"](${bbox});
    nwr["natural"~"wood|scrub|heath|grassland|bare_rock|sand|shingle"](${bbox});
    nwr["leisure"~"park|garden|pitch|playground|golf_course|square|nature_reserve"](${bbox});
    nwr["amenity"~"parking|university|hospital|school"](${bbox}); 
  `,
  /** * [Pass 2] water-layer (수계)
   * 지형 위에 물을 덮습니다.
   */
  WATER: (bbox: string) => `
    nwr["natural"~"water|cliff|coastline"](${bbox});
    nwr["waterway"~"river|riverbank|canal|stream|drain|dock"](${bbox});
    nwr["water"~"river|oxbow|lake|reservoir|basin|pond"](${bbox});
  `,
  /** * [Pass 3] transport-layer (도로 및 철도)
   * 이동 가능한 경로를 생성합니다.
   */
  TRANSPORT: (bbox: string) => `
    nwr["highway"~"motorway|trunk|primary|secondary|tertiary|residential|service|unclassified|pedestrian|living_street|footway|cycleway|path"](${bbox});
    nwr["highway"~".*_link"](${bbox});
    nwr["area:highway"](${bbox});
    nwr["railway"~"rail|subway|light_rail|tram"](${bbox});
    nwr["junction"](${bbox});
  `,
  /** * [Pass 4 structure-layer (건물 및 인프라)
   * 가장 높은 우선순위로 캔버스 최상단을 덮습니다.
   */
  STRUCTURE: (bbox: string) => `
    nwr["building"](${bbox});
    nwr["building:part"](${bbox});
    nwr["man_made"~"pier|bridge|embankment|groyne"](${bbox});
    nwr["bridge"="yes"](${bbox});
    nwr["tunnel"="yes"](${bbox});
  `,
} as const;
