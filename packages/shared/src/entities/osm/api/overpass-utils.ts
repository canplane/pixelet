/**
 * @file overpass-utils.ts
 * Overpass QL(Query Language) 생성을 위한 유틸리티 함수 모음입니다.
 */

export type MapQueriesParams = {
  bbox: string; // 형식: "south,west,north,east" (위경도 경계 문자열)
};

/**
 * 쿼리 생성 헬퍼 함수
 * 필터 문자열을 입력받아 실행 가능한 Overpass QL 전체 쿼리로 포장합니다.
 * * [Output Mode Analysis & Strategy]
 * 1. out body geom qt;
 * - 서버가 Geometry를 직접 조립하여 전송. 클라이언트 부하가 가장 적음.
 * - 로컬 도커 환경에서는 유리하나, 공용 API 서버에서는 복잡한 데이터 누락 위험이 있음.
 * 2. out body geom center qt;
 * - Geometry와 중심점(Center)을 서버에서 동시 연산.
 * - 연산 부하가 매우 높아 대규모 구역(건물군) 요청 시 서버가 응답에서 좌표 필드를 생략하는 현상 발생.
 * 3. (._; > ;); out body qt; (현재 채택)
 * - Recurse Down( > )을 통해 필터링된 요소와 연관된 모든 원천 노드(Node) 좌표를 개별 포함.
 * - 서버 연산 부담이 없어 데이터 유실 위험이 0%에 수렴함.
 * - Node.js 서버(또는 클라이언트)의 nodeCache 로직을 통해 100% 무결한 인프라 복원 가능.
 * * @param filterContent - nwr[...] 형태의 순수 필터 문자열
 * @param timeout - 쿼리 타임아웃 (초 단위, 기본값 90)
 */
export const create_query = (filterContent: string, timeout = 90) => {
  const sanitized = filterContent.replace(/\s+/g, ' ').trim();

  // 데이터 무결성 최우선: 원천 노드를 모두 가져와서 파서(nodeCache)가 직접 조립하도록 유도
  // return `[out:json][timeout:${timeout}];(${sanitized});(._; > ;);out body qt;`;
  return `[out:json][timeout:${timeout}];(${sanitized});out body geom qt;`;
};

/**
 * [debug-only] 특정 영역의 모든 원천 데이터를 추출합니다.
 */
export const get_raw_all_query = ({ bbox }: MapQueriesParams): string => `
      [out:json][timeout:180][maxsize:200000000];
      (node(${bbox}); way(${bbox}); relation(${bbox}););
      (._; > ;);
      out body qt;
    `;
