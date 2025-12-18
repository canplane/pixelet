import type { OverpassResponse } from '../model/types';
import { OSM_CONFIG } from '../config/osm-config';

/**
 * 특정 좌표 기준 반경 내 도로 데이터를 가져오는 API 함수
 */
export async function fetchOsmData(query: string): Promise<OverpassResponse> {
  // 1. 서버 목록 무작위 섞기 (로드 밸런싱)
  const shuffled = [...OSM_CONFIG.ENDPOINTS].sort(() => Math.random() - 0.5);

  let lastError: Error | null = null;

  // 2. 살아있는 서버를 찾을 때까지 순차 시도
  for (const url of shuffled) {
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 12000); // 12초 타임아웃

      console.log('fetchOsmData request', url, query);

      // URL에 쿼리를 붙이지 않고, Body에 담아 보냅니다.
      const response = await fetch(url, {
        method: 'POST',
        // 쿼리를 'data=' 형식으로 인코딩하여 보냅니다.
        body: `data=${encodeURIComponent(query)}`,
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      // 429 Too Many Requests나 504 Gateway Timeout 등 체크
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const data = await response.json();

      console.log('fetchOsmData response', data);

      // 데이터가 비어있지 않은지 최소한의 검증
      if (!data || !data.elements) {
        throw new Error('Invalid Data Format');
      }

      return data;
    } catch (error: any) {
      // 타임아웃인지, 네트워크 단절인지, 서버 에러인지 구분해서 로그 남기기
      const errorMessage =
        error.name === 'AbortError' ? 'Timeout (12s)' : error.message;
      console.warn(
        `⚠️ Overpass 실패 (${url}): ${errorMessage}. 다음 서버 시도 중...`,
      );
      lastError = new Error(errorMessage);
    }
  }

  // 모든 서버가 실패했을 경우
  throw new Error(`모든 지리 데이터 서버 응답 실패: ${lastError?.message}`);
}
