// 1. 프로젝트 공통 타입 및 설정 (Shared / Feature Config)
import { OSM_CONFIG } from '../config/osm-config';
import type { OverpassResponse } from '../model/types';

/**
 * Overpass API 요청을 수행합니다.
 * 설정된 모드('local' | 'public')에 따라 최적화된 전략을 사용합니다.
 */
export async function requestOverpass(
  query: string,
): Promise<OverpassResponse> {
  // [Mode 1] 로컬 서버 모드 (Super Fast & Unlimited)
  if (OSM_CONFIG.API_MODE === 'local') {
    return fetchFromUrl(OSM_CONFIG.LOCAL_ENDPOINT, query, 'Local Docker');
  }

  // [Mode 2] 공용 서버 모드 (Legacy: Load Balancing & Fallback)
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

/**
 * 실제 fetch를 수행하는 내부 헬퍼 함수
 */
async function fetchFromUrl(
  url: string,
  query: string,
  label: string,
): Promise<OverpassResponse> {
  const startTime = performance.now();
  const controller = new AbortController();
  const timeoutId = setTimeout(
    () => controller.abort(),
    OSM_CONFIG.REQUEST_TIMEOUT,
  );

  try {
    const response = await fetch(url, {
      method: 'POST',
      body: `data=${encodeURIComponent(query)}`, // 헤더 없이 body만으로도 보통 동작함
      signal: controller.signal,
    });

    clearTimeout(timeoutId);

    if (!response.ok) {
      throw new Error(`HTTP ${response.status} ${response.statusText}`);
    }

    const data = (await response.json()) as OverpassResponse;
    const duration = ((performance.now() - startTime) / 1000).toFixed(3);

    if (!data || !data.elements) {
      throw new Error('Invalid Response Data');
    }

    // [LOG] 로컬일 때는 너무 빠르므로 로그가 스팸이 되지 않게 필요시에만 출력하거나 간소화
    console.debug(
      `✅ [${label}] Fetched ${data.elements.length} items in ${duration}s`,
    );

    return data;
  } catch (error) {
    clearTimeout(timeoutId);
    throw error;
  }
}
