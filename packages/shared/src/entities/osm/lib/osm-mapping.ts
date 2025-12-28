/**
 * @file osm-mapping.ts
 * OSM 데이터를 복셀 시스템으로 변환할 때 필요한 도메인 지식을 정의합니다.
 */

/**
 * 도로 종류별 기본 차선 수(Lanes) 매핑
 * 데이터에 lanes 태그가 없을 경우 이 값을 기본값으로 사용하여 도로의 위계를 시각화합니다.
 */
export const ROAD_THICKNESS_MAP: Record<string, number> = {
  motorway: 4, // 고속도로 (왕복이 아닌 편도 기준 혹은 시각적 굵기)
  trunk: 3, // 간선도로
  primary: 3, // 주요 도로
  secondary: 2, // 보조 도로
  tertiary: 2, // 지역 도로
  residential: 1, // 주거지 도로
  service: 1, // 서비스 도로
  unclassified: 1, // 미분류
  path: 0.5, // 오솔길 (보강)
  footway: 0.5, // 보도 (보강)
  rail: 1, // 철도
};

// // 렌더링 시 활용 예시
// const getRoadColor = (element: OverpassElement) => {
//   const type = element.tags?.highway;
//   if (type === 'motorway') return '#ff9900'; // 주황색 (고속도로)
//   if (type === 'footway') return '#cccccc';   // 회색 (보도)
//   return '#ffffff';                           // 기본 흰색
// };
