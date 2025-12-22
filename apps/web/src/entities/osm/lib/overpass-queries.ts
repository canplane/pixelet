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
   * 전체 지형지물 렌더링용 쿼리 (Ultra Heavy)
   * * 특정 태그 구분 없이 지정된 반경 내의 모든 기하 구조를 가져옵니다.
   */
  GEOMETRY_ALL: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    return `
    [out:json][timeout:60]; // 🚀 모든 데이터를 가져오므로 타임아웃을 늘림
    (
      // 해당 반경 내의 모든 노드, 웨이, 릴레이션을 한 번에 수집
      node(around:${radius},${lat},${lng});
      way(around:${radius},${lat},${lng});
      relation(around:${radius},${lat},${lng});
    );
    // 'body'는 속성을, 'geom'은 좌표 정보를 포함합니다.
    out body geom;
    >;
    out skel qt;
    `;
  },

  /**
   * 전체 지형지물 렌더링용 쿼리 (Heavy)
   *
   * 도로와 모든 건물을 가져옵니다. (기존 로직 유지)
   */
  GEOMETRY_ALL_1: ({ location, radius }: MapQueriesParams) => {
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

  GEOMETRY_MAJOR: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;

    // 주요 인프라만 콤팩트하게 가져오는 버전
    const majorBuildingFilter =
      '"building"~"commercial|retail|office|public|university|hospital|government|stadium|museum"';
    const majorRoadFilter =
      '"highway"~"motorway|trunk|primary|secondary|tertiary"'; // residential 제외 고려

    return `
      [out:json][timeout:30];
      (
        way[${majorRoadFilter}](around:${radius},${lat},${lng});
        way[${majorBuildingFilter}](around:${radius},${lat},${lng});
        relation[${majorBuildingFilter}](around:${radius},${lat},${lng});
        
        // 물과 큰 공원은 지도의 '배경'이 되므로 유지하되, 면적이 큰 것 위주로
        way["natural"="water"](around:${radius},${lat},${lng});
        way["leisure"="park"](around:${radius},${lat},${lng});
      );
      out body geom; // 좌표 정보를 포함한 본체만 깔끔하게 출력
    `;
  },

  GEOMETRY_MAJOR_1: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;

    // 1. 주요 랜드마크 및 대형 건물 (주거용 제외)
    const buildingFilter =
      '"building"~"commercial|retail|office|public|civic|hospital|university|government|industrial|transportation"';

    // 2. 주요 도로망 (보행자 도로는 제외하여 가독성 확보)
    const roadFilter =
      '"highway"~"motorway|trunk|primary|secondary|tertiary|residential"';

    // 3. 물과 녹지 (공간을 채워주는 중요한 레이어)
    const waterFilter = '"natural"~"water"';
    const waterwayFilter = '"waterway"~"river|canal"';
    const landuseFilter =
      '"landuse"~"park|grass|forest|recreation_ground|cemetery"';
    const leisureFilter = '"leisure"~"park|garden|golf_course"';

    return `
      [out:json][timeout:30];
      (
        // 도로
        way[${roadFilter}](around:${radius},${lat},${lng});
        
        // 건물 (Way & Relation 모두 포함)
        way[${buildingFilter}](around:${radius},${lat},${lng});
        relation[${buildingFilter}](around:${radius},${lat},${lng});
        
        // 물 레이어
        way[${waterFilter}](around:${radius},${lat},${lng});
        relation[${waterFilter}](around:${radius},${lat},${lng});
        way[${waterwayFilter}](around:${radius},${lat},${lng});
        
        // 녹지/공원 레이어
        way[${landuseFilter}](around:${radius},${lat},${lng});
        way[${leisureFilter}](around:${radius},${lat},${lng});
        relation[${landuseFilter}](around:${radius},${lat},${lng});
      );
      out body geom;
      >;
      out skel qt;
    `;
  },

  /**
   * 절충안 쿼리 (Balanced)
   *
   * 모든 도로는 가져오되, 건물은 '주요 건물'만 필터링해서 가져옵니다.
   * 주택(house, apartments)을 제외한 상업, 공공, 랜드마크 위주로 구성됩니다.
   */
  GEOMETRY_MAJOR_2: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    // 주거용 건물 제외, 주요 도로(주간선, 보조간선 등) 위주 구성
    const buildingFilter =
      '"building"~"commercial|retail|public|civic|hospital|university|church|cathedral|government"';
    // 골목길(service, footway 등) 제외 필터
    const roadFilter =
      '"highway"~"motorway|trunk|primary|secondary|tertiary|residential"';

    return `
    [out:json][timeout:25];
    (
      way[${roadFilter}](around:${radius},${lat},${lng});
      way[${buildingFilter}](around:${radius},${lat},${lng});
      relation[${buildingFilter}](around:${radius},${lat},${lng});
    );
    out body geom;
    `;
  },

  /**
   * 도로 전용 쿼리 (Ultra Light)
   *
   * 건물 데이터를 완전히 배제하고 도로망만 가져옵니다.
   * 성능이 가장 좋으며 렌더링 부하가 거의 없습니다.
   */
  GEOMETRY_ROADS_ALL: ({ location, radius }: MapQueriesParams) => {
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
   * 주요 도로 전용 쿼리 (Mobile Optimized)
   *
   * 건물은 아예 제외하고, 골목길(service, footway)을 뺀 주요 간선 도로만 가져옵니다.
   * 모바일에서 지도를 빠르게 훑을 때 가장 부드러운 프레임을 보장합니다.
   */
  GEOMETRY_ROADS_MAJOR: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    // 간선도로, 보조간선도로 등 주요 네트워크만 포함 (residential은 선택사항)
    const majorRoadFilter =
      '"highway"~"motorway|trunk|primary|secondary|tertiary"';

    return `
    [out:json][timeout:20];
    (way[${majorRoadFilter}](around:${radius},${lat},${lng}););
    out body geom;
    >;
    out skel qt;
    `;
  },

  POI_ALL: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    return `
    [out:json][timeout:30];
    (
      // 1. 생활 밀착형 랜드마크 (카페, 음식점 등 인기 스팟)
      node["amenity"~"cafe|restaurant|pub|bar|bakery"](around:${radius},${lat},${lng});
      way["amenity"~"cafe|restaurant|pub|bar|bakery"](around:${radius},${lat},${lng});

      // 2. 관광 및 문화 거점 (박물관, 예술 센터, 전망대)
      node["tourism"~"attraction|viewpoint|museum|art_gallery"](around:${radius},${lat},${lng});
      node["amenity"~"arts_centre|theatre|cinema"](around:${radius},${lat},${lng});

      // 3. 쇼핑 및 교통 허브 (대형 마트, 지하철역, 백화점)
      node["shop"~"mall|department_store"](around:${radius},${lat},${lng});
      way["shop"~"mall|department_store"](around:${radius},${lat},${lng});
      node["railway"="station"](around:${radius},${lat},${lng});

      // 4. 대규모 휴식 및 역사적 장소
      way["leisure"~"park|garden"](around:${radius},${lat},${lng});
      node["historic"~"memorial|monument|castle"](around:${radius},${lat},${lng});
    );
    out center; // 'center' 키워드를 사용해 영역 데이터도 하나의 좌표로 압축해서 가져옵니다.
  `;
  },

  /**
   * 🚀 주요 POI 전용 쿼리 (Mobile Optimized)
   * 수만 개의 카페/식당 대신, 길 찾기의 기준이 되는 주요 거점만 가져옵니다.
   * 역, 관공서, 대형 쇼핑몰, 주요 문화시설 위주입니다.
   */
  POI_MAJOR: ({ location, radius }: MapQueriesParams) => {
    const { lng, lat } = location;
    return `
    [out:json][timeout:20];
    (
      // 1. 교통 허브 (역, 터미널) - 가장 중요
      node["railway"~"station|subway_entrance"](around:${radius},${lat},${lng});
      
      // 2. 주요 공공기관 및 대형 병원
      node["amenity"~"townhall|hospital|university"](around:${radius},${lat},${lng});
      
      // 3. 대형 상업 시설 및 랜드마크
      node["shop"~"mall|department_store"](around:${radius},${lat},${lng});
      node["tourism"~"museum|viewpoint|attraction"](around:${radius},${lat},${lng});
      
      // 4. 대규모 녹지 (공원 등은 영역의 중심점만)
      way["leisure"="park"](around:${radius},${lat},${lng});
    );
    out center; 
    `;
  },
} as const;
