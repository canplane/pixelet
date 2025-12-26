/**
 * @file osm-mapping.ts
 * OSM 데이터를 복셀 시스템으로 변환할 때 필요한 도메인 지식을 정의합니다.
 */

/**
 * 도로 종류별 기본 두께 매핑
 */
export const ROAD_THICKNESS_MAP: Record<string, number> = {
  motorway: 4,
  trunk: 3,
  primary: 3,
  secondary: 2,
  tertiary: 2,
  residential: 1,
  service: 1,
  unclassified: 1,
  rail: 1,
};

// // 렌더링 시 활용 예시
// const getRoadColor = (element: OverpassElement) => {
//   const type = element.tags?.highway;
//   if (type === 'motorway') return '#ff9900'; // 주황색 (고속도로)
//   if (type === 'footway') return '#cccccc';   // 회색 (보도)
//   return '#ffffff';                           // 기본 흰색
// };
