/**
 * [map-queries]
 * OpenStreetMap(OSM) 데이터를 추출하기 위한 Overpass QL 쿼리 셋입니다.
 * 타일 격자 시스템과의 정합성을 위해 모든 쿼리는 BBox(Bounding Box) 방식을 채택합니다.
 */

type MapQueriesParams = {
  bbox: string; // 형식: "south,west,north,east" (위경도 경계 문자열)
};

export const MAP_QUERIES = {
  /**
   * [geometry-all]
   * 타일 내 모든 기하 데이터(Node, Way, Relation)를 필터 없이 전수 수집합니다.
   * 주로 디버깅 및 특정 지역의 데이터 구조 전수 조사용으로 사용됩니다.
   */
  GEOMETRY_ALL: ({ bbox }: MapQueriesParams) => {
    return `
    [out:json][timeout:120][maxsize:200000000];
    (
      node(${bbox});
      way(${bbox});
      relation(${bbox});
    );
    out body geom;
    `;
  },

  GEOMETRY_MAJOR: ({ bbox }: MapQueriesParams): string => {
    return `
      [out:json][timeout:180]; // 복잡한 관계 처리를 위해 타임아웃 확장
      (
        // 1. 정밀 수계 (Hydrology)
        // 면적형 수계(호수, 강폭)와 선형 수계(운하, 스트림)를 모두 포함하되 중심선 데이터 병합
        nwr["natural"="water"](${bbox});
        nwr["waterway"~"river|riverbank|canal|stream|drain"](${bbox});
        nwr["water"~"river|oxbow|lake|reservoir|basin"](${bbox});
  
        // 2. 계층형 도로망 (Hierarchical Transport)
        // lanes, width, surface 정보를 함께 가져와 베이킹 두께를 동적으로 결정 가능하게 함
        nwr["highway"~"motorway|trunk|primary|secondary|tertiary|residential|service|unclassified|pedestrian|living_street"](${bbox});
        nwr["highway"~".*_link"](${bbox});
        // 보행자 전용 대형 광장 및 보행로 (면적형 도로 대응)
        nwr["highway"="pedestrian"]["area"="yes"](${bbox});
        nwr["area:highway"](${bbox});
  
        // 3. 철도 및 특수 교통 인프라
        // 터널과 교량을 구분하여 복셀 높이값(Y) 계산에 활용
        nwr["railway"~"rail|subway|light_rail|tram"](${bbox});
        nwr["bridge"="yes"](${bbox});
        nwr["tunnel"="yes"](${bbox});
  
        // 4. 구조적 건물 및 수직 요소 (Vertical Structures)
        // 높이 데이터 정밀 추출: building:levels, building:height, min_height 등
        (
          // 주요 상업/공공 랜드마크
          nwr["building"~"commercial|retail|office|public|university|hospital|government|stadium|museum|hotel|train_station"](${bbox});
          // 높이 정보가 명시된 모든 건물 (주거용 포함 시각적 스카이라인 형성)
          nwr["building"]["height"](${bbox});
          nwr["building"]["building:levels"](${bbox});
          // 지붕 형태 및 파트별 데이터 (복잡한 건물 메쉬 생성용)
          nwr["building:part"](${bbox});
        );
        
        // 5. 정밀 토지 용도 (Landuse & Landcover)
        // 복셀 색상 결정을 위한 세부 피복 데이터
        nwr["leisure"~"park|garden|pitch|playground|golf_course"](${bbox});
        nwr["landuse"~"forest|grass|recreation_ground|residential|commercial|industrial|cemetery|allotments|quarry"](${bbox});
        nwr["natural"~"wood|scrub|heath|grassland|bare_rock|sand"](${bbox});
      );
      // [고도화] qt(QuadTile) 정렬로 데이터 접근 속도 향상, body geom으로 좌표 무결성 확보
      out body geom qt;
    `;
  },

  // /**
  //  * [geometry-major]
  //  * 지형 생성(Voxel Baking) 및 메인 렌더링에 필요한 핵심 인프라 데이터를 로드합니다.
  //  * 수계, 도로망, 랜드마크 건물, 토지 피복 데이터를 위계별로 필터링합니다.
  //  */
  // GEOMETRY_MAJOR: ({ bbox }: MapQueriesParams) => {
  //   return `
  //   [out:json][timeout:120];
  //   (
  //     // 1. 수계 (Water): 강, 호수, 운하 등 지형의 기본틀
  //     nwr["natural"="water"](${bbox});
  //     nwr["waterway"~"river|riverbank|canal|stream"](${bbox});

  //     // 2. 교통망 (Transport): 고속도로부터 지하철 노선까지의 선형 데이터
  //     nwr["highway"~"motorway|trunk|primary|secondary|tertiary|residential|service|unclassified|motorway_link|trunk_link|primary_link|secondary_link"](${bbox});
  //     way["railway"~"rail|subway|light_rail"](${bbox});

  //     // 3. 인프라 구조물: 교량(bridge) 및 면적형 도로 데이터
  //     nwr["area:highway"](${bbox});
  //     nwr["highway"]["area"="yes"](${bbox});
  //     way["bridge"="yes"](${bbox});

  //     // 4. 주요 건물: 랜드마크 및 고층 빌딩 (최적화를 위해 단순 주거용 제외)
  //     nwr["building"~"commercial|retail|office|public|university|hospital|government|stadium|museum"](${bbox});
  //     nwr["building"]["height"~"^[3-9][0-9]|^[1-9][0-9][0-9]"](${bbox});

  //     // 5. 토지 피복: 녹지, 공원, 산업/상업지구 배경 레이어
  //     nwr["leisure"="park"](${bbox});
  //     nwr["landuse"~"forest|grass|recreation_ground|residential|commercial|industrial"](${bbox});
  //   );
  //   out body geom qt;
  //   `;
  // },

  /**
   * [poi-all]
   * 격자 내 식음료, 관광, 문화 시설 등 광범위한 관심 지점(POI)을 수집합니다.
   * 생활 밀착형 정보 표시 및 검색 인덱싱용으로 사용됩니다.
   */
  POI_ALL: ({ bbox }: MapQueriesParams) => {
    return `
    [out:json][timeout:60];
    (
      node["amenity"~"cafe|restaurant|pub|bar|bakery"](${bbox});
      node["tourism"~"attraction|viewpoint|museum|art_gallery"](${bbox});
      node["amenity"~"arts_centre|theatre|cinema"](${bbox});
      node["shop"~"mall|department_store"](${bbox});
      node["railway"="station"](${bbox});
      node["historic"~"memorial|monument|castle"](${bbox});
      way["leisure"~"park|garden"](${bbox});
    );
    out center;
    `;
  },

  /**
   * [poi-major]
   * 길 찾기와 공간 인지의 기준이 되는 핵심 거점(교통 허브, 공공기관 등)만 추출합니다.
   * 데이터 트래픽을 절감하고 화면의 가독성을 확보하기 위한 모바일 최적화 쿼리입니다.
   */
  POI_MAJOR: ({ bbox }: MapQueriesParams) => {
    return `
    [out:json][timeout:30];
    (
      node["railway"~"station|subway_entrance"](${bbox});
      node["junction"="yes"](${bbox});
      node["amenity"~"townhall|hospital|university"](${bbox});
      node["shop"~"mall|department_store"](${bbox});
      node["tourism"~"museum|viewpoint|attraction"](${bbox});
      way["leisure"="park"](${bbox});
    );
    out center; 
    `;
  },
} as const;
