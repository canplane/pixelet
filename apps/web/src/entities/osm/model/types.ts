/**
 * Node (점): 위도(lat)와 경도(lon)를 직접 가집니다.
 * Way (선/면): 여러 노드의 ID 목록(nodes: [123, 456, ...])만 가지고 있으며, 개별 노드의 좌표는 포함하지 않는 것이 기본 응답값입니다. [1, 2]
 * Relation (관계): 여러 노드, 선, 또는 다른 관계들의 묶음이므로 자체 좌표가 없습니다.
 */

/**
 * Overpass API의 개별 요소 타입
 */
export type ElementType = 'node' | 'way' | 'relation';

/**
 * 위경도 좌표 객체
 */
export interface LatLng {
  lat: number;
  lon: number;
}

/**
 * Overpass API 응답의 개별 요소 (Element)
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
