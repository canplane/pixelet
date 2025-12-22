// 이 파일은 OpenStreetMap(OSM)의 Overpass API로부터 수신되는 지리 데이터의 구조를 정의합니다.

/**
 * Overpass API의 개별 요소 타입
 * - node: 특정 지점(Point)의 위경도 정보를 가진 최소 단위
 * - way: 노드들이 순서대로 연결된 선(Line) 또는 폐곡면(Polygon) 데이터
 * - relation: 노드, 웨이, 다른 릴레이션을 묶어 하나의 논리적 그룹을 형성 (예: 복잡한 경계선)
 */
export type ElementType = 'node' | 'way' | 'relation';

/**
 * 위경도 좌표 객체 (OSM 표준 규격: lat, lon)
 */
export interface LatLng {
  lat: number;
  lon: number;
}

/**
 * OSM 태그들을 성격별로 구체화하여 정의합니다.
 * parseOsmData에서 이 태그들을 기준으로 MapInfra와 MapPoi를 분류합니다.
 */
export interface OsmTags {
  /** 공통: 장소나 시설의 고유 이름 */
  name?: string;

  // --- MapInfra 관련 태그 (물리적 형체 위주) ---
  /** 건물 여부 및 종류 */
  building?: string;
  /** 건물의 층수 (높이 계산용) */
  'building:levels'?: string;
  /** 명시적인 건물 높이(m) */
  height?: string;
  /** 도로의 등급 (motorway, primary, residential 등) */
  highway?: string;
  /** 자연 지형 (water, wood, sand 등) */
  natural?: string;
  /** 수계 상세 (river, canal 등) */
  waterway?: string;
  /** 토지 용도 (grass, forest, commercial, residential 등) */
  landuse?: string;

  // --- MapPoi 관련 태그 (정보 및 장소 위주) ---
  /** 생활 편의 시설 (cafe, restaurant, hospital 등) */
  amenity?: string;
  /** 상점 종류 (mall, bakery, supermarket 등) */
  shop?: string;
  /** 관광 명소 및 문화 시설 (museum, viewpoint, attraction 등) */
  tourism?: string;
  /** 역사적 가치가 있는 유적지 (monument, castle, ruins 등) */
  historic?: string;
  /** 철도 및 대중교통 거점 (station 등) */
  railway?: string;

  /** 가변적인 모든 태그 허용 */
  [key: string]: string | undefined;
}

/**
 * Overpass API 응답의 개별 요소 (Element)
 * 쿼리 시 out body, out center, out geom 등의 옵션 조합에 따라 속성이 동적으로 채워집니다.
 */
export interface OverpassElement {
  type: ElementType;
  id: number;

  /** node 타입일 때 존재하는 개별 위경도 */
  lat?: number;
  lon?: number;

  /** way나 relation의 중심 좌표 (out center 옵션 시 생성) - MapPoi 렌더링 시 기준점 */
  center?: LatLng;

  /** way의 전체 경로 좌표 배열 (out geom 옵션 시 생성) - MapInfra 3D Mesh 생성 시 사용 */
  geometry?: LatLng[];

  /** way를 구성하는 노드들의 ID 배열 */
  nodes?: number[];

  /** 장소의 속성을 담고 있는 키-값 쌍 (MapInfra/MapPoi 분류 기준) */
  tags?: OsmTags;
}

/**
 * Overpass API 전체 응답 구조
 */
export interface OverpassResponse {
  version?: number;
  generator?: string;
  osm3s?: {
    timestamp_osm_base: string;
    copyright: string;
  };
  /** 실제 지리 정보 데이터를 담고 있는 배열 */
  elements: OverpassElement[];
}

// // 렌더링 시 활용 예시
// const getRoadColor = (element: OverpassElement) => {
//   const type = element.tags?.highway;
//   if (type === 'motorway') return '#ff9900'; // 주황색 (고속도로)
//   if (type === 'footway') return '#cccccc';   // 회색 (보도)
//   return '#ffffff';                           // 기본 흰색
// };
