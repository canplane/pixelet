/**
 * @file overpass-utils.ts
 */

export type MapQueriesParams = {
  bbox: string; // 형식: "south,west,north,east" (위경도 경계 문자열)
};

/**
 * 쿼리 생성 헬퍼 함수
 * 필터 문자열을 입력받아 실행 가능한 Overpass QL 전체 쿼리로 포장합니다.
 * @param filterContent - nwr[...] 형태의 순수 필터 문자열
 * @param timeout - 쿼리 타임아웃 (초 단위, 기본값 90)
 */
export const create_query = (filterContent: string, timeout = 90) => {
  return `[out:json][timeout:${timeout}];
    (
      ${filterContent}
    );
    out body geom qt;`;
};

/**
 * [debug-only] 특정 영역의 모든 원천 데이터를 추출합니다.
 * osm-geometry-queries 등에서 필터링된 결과와 비교 분석할 때 사용합니다.
 */
export const get_raw_all_query = ({ bbox }: MapQueriesParams): string => `
    [out:json][timeout:180][maxsize:200000000];
    (node(${bbox}); way(${bbox}); relation(${bbox}););
    out body geom;
  `;
