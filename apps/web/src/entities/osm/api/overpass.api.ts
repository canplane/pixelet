// 1. 프로젝트 공통 타입 및 설정 (Shared / Feature Config)
import { OSM_CONFIG } from '../config/osm-config';
import type { OverpassResponse } from '../model/types';

/**
 * Overpass API 서버군에 지리 데이터 쿼리를 요청합니다.
 * 로드 밸런싱을 위해 가용 서버 중 하나를 무작위로 선택하며, 장애 발생 시 자동으로 Fallback을 시도합니다.
 */
export async function requestOverpass(
  query: string,
): Promise<OverpassResponse> {
  // 1. 서버 목록 무작위 셔플 (특정 서버 부하 집중 방지)
  const shuffledServers = [...OSM_CONFIG.ENDPOINTS].sort(
    () => Math.random() - 0.5,
  );

  let lastError: Error | null = null;

  // 2. 가용 서버 순차 시도
  for (const url of shuffledServers) {
    const startTime = performance.now();
    const serverName = new URL(url).hostname;

    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 12000); // 12초 타임아웃

      // [LOG] 요청 시작 그룹
      console.groupCollapsed(`🌐 [Overpass Request] -> ${serverName}`);
      console.log('%cQuery:', 'color: #007acc; font-weight: bold;', query);
      console.groupEnd();

      // 3. API 요청 수행
      const response = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: `data=${encodeURIComponent(query)}`,
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        throw new Error(`HTTP Error: ${response.status}`);
      }

      const data = (await response.json()) as OverpassResponse;
      const endTime = performance.now();
      const duration = ((endTime - startTime) / 1000).toFixed(2);

      if (!data || !data.elements) {
        throw new Error('Invalid Response: Missing elements');
      }

      // [LOG] 응답 성공 및 바디 출력
      console.group(
        `✅ [Overpass Success] ${data.elements.length} elements in ${duration}s`,
      );
      console.log('%cServer:', 'color: #888;', url);
      console.log(
        '%cFull Response Body:',
        'color: #4caf50; font-weight: bold;',
        data,
      );
      console.groupEnd();

      return data;
    } catch (error: any) {
      const duration = ((performance.now() - startTime) / 1000).toFixed(2);
      const errorMessage =
        error.name === 'AbortError' ? 'Timeout (12s)' : error.message;

      console.warn(
        `⚠️ [Overpass Retry] ${url} 실패 (${duration}s): ${errorMessage}`,
      );
      lastError = new Error(errorMessage);
    }
  }

  throw new Error(`[Overpass API] 모든 서버 응답 실패: ${lastError?.message}`);
}
