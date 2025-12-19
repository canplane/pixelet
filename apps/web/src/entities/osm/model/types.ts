// 이 파일은 OpenStreetMap(OSM)의 Overpass API로부터 수신되는 지리 데이터의 구조를 정의합니다.

/**
 * Overpass API의 개별 요소 타입
 * - node: 점 데이터 (위경도 직접 보유)
 * - way: 선 또는 면 데이터 (노드들의 집합)
 * - relation: 여러 요소들의 논리적 묶음
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
 * Overpass API 응답의 개별 요소 (Element)
 * API 쿼리 옵션(out center, out geom 등)에 따라 포함되는 속성이 달라집니다.
 */
export interface OverpassElement {
  type: ElementType;
  id: number;

  /**
   * 'node' 타입일 때 존재 (직접 좌표)
   */
  lat?: number;
  lon?: number;

  /**
   * 'way' 또는 'relation' 타입에 'out center' 옵션을 썼을 때 존재
   */
  center?: LatLng;

  /**
   * 'way' 또는 'relation' 타입에 'out geom' 옵션을 썼을 때 존재 (폴리곤/선 렌더링용)
   */
  geometry?: LatLng[];

  /**
   * 'way'일 때 해당 선을 구성하는 노드 ID 배열 (out body 시 포함)
   */
  nodes?: number[];

  /**
   * 장소의 속성 정보 (이름, 시설 종류 등)
   */
  tags?: {
    name?: string;
    amenity?: string;
    building?: string;
    highway?: string;
    [key: string]: string | undefined; // 가변적인 모든 태그 허용
  };
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
  elements: OverpassElement[];
}

// // 렌더링 시 활용 예시
// const getRoadColor = (element: OverpassElement) => {
//   const type = element.tags?.highway;
//   if (type === 'motorway') return '#ff9900'; // 주황색 (고속도로)
//   if (type === 'footway') return '#cccccc';   // 회색 (보도)
//   return '#ffffff';                           // 기본 흰색
// };
