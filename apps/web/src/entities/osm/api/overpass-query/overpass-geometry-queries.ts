/**
 * @file overpass-geometry-queries.ts
 * 복셀 지형 및 물리적 구조물 생성을 위한 Overpass QL 쿼리 셋입니다.
 */

/**
 * OSM 태그 그룹 분리 (Reusable Filter Sets)
 */
export const GEOMETRY_FILTERS = {
  /** * [Pass 1] 수계 (Water) */
  WATER: (bbox: string) => `
    nwr["natural"~"water|cliff|coastline"](${bbox});
    nwr["waterway"~"river|riverbank|canal|stream|drain|dock"](${bbox});
    nwr["water"~"river|oxbow|lake|reservoir|basin|pond"](${bbox});
  `,
  /** * [Pass 2] 지형 (Base Land, Natural, Landuse) */
  BASE: (bbox: string) => `
    nwr["landuse"~"forest|grass|residential|commercial|industrial|cemetery|allotments|quarry|construction|meadow"](${bbox});
    nwr["natural"~"wood|scrub|heath|grassland|bare_rock|sand|shingle|fell"](${bbox});
    nwr["leisure"~"park|garden|pitch|playground|golf_course|square|nature_reserve|track|stadium"](${bbox});
    nwr["amenity"~"parking|university|hospital|school|college"](${bbox});
    nwr["campus"](${bbox});
  `,
  /** * [Pass 3] 교통 (Transport) */
  TRANSPORT: (bbox: string) => `
    nwr["highway"~"motorway|trunk|primary|secondary|tertiary|residential|service|unclassified|pedestrian|living_street|footway|cycleway|path"](${bbox});
    nwr["highway"~".*_link"](${bbox});
    nwr["area:highway"](${bbox});
    nwr["railway"~"rail|subway|light_rail|tram"](${bbox});
    nwr["junction"](${bbox});
  `,
  /** * [Pass 4] 구조물 (Structure) */
  STRUCTURE: (bbox: string) => `
    nwr["building"](${bbox});
    nwr["building:part"](${bbox});
    nwr["man_made"~"pier|bridge|embankment|groyne"](${bbox});
    nwr["bridge"="yes"](${bbox});
    nwr["tunnel"="yes"](${bbox});
  `,
} as const;
