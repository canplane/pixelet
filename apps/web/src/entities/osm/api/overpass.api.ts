// 1. 프로젝트 공통 타입 및 설정 (Shared / Feature Config)
import { OSM_CONFIG } from '../config/osm-config';
import type { OverpassResponse } from './overpass-types';
import { randomDelay } from '@pixelet/shared/shared/lib/aync-utils'; // 기존에 사용하던 유틸리티 활용
import { withSharedPromise } from '@pixelet/shared/shared/lib/cache-utils';

const { PUBLIC_API_ENDPOINTS } = OSM_CONFIG;

/**
 * Overpass API 요청을 수행합니다.
 * 설정된 모드('local' | 'public')에 따라 최적화된 전략을 사용합니다.
 * * [Strategy]
 * - 공용 서버 모드에서는 데이터 누락(remark) 발생 시 딜레이 후 재시도합니다.
 * - HTTP 429(Too Many Requests) 또는 서버 타임아웃 발생 시 다음 가용 서버로 전환합니다.
 */
export async function requestOverpass(
  query: string,
): Promise<OverpassResponse> {
  return withSharedPromise('overpass', query, async () => {
    // [Mode 2] 공용 서버 모드 (Legacy: Load Balancing & Fallback)
    const shuffledServers = [...PUBLIC_API_ENDPOINTS].sort(
      () => Math.random() - 0.5,
    );
    let lastError: Error | null = null;

    // 2. 가용 서버 순차 시도
    for (const url of shuffledServers) {
      const startTime = performance.now();
      const serverName = new URL(url).hostname;

      // 서버당 최대 재시도 횟수 (누락 응답 대응용)
      const MAX_RETRY_PER_SERVER = 2;

      for (let attempt = 1; attempt <= MAX_RETRY_PER_SERVER; attempt++) {
        try {
          const controller = new AbortController();
          const timeoutId = setTimeout(() => controller.abort(), 15000); // 공용은 조금 더 넉넉히 15초

          // [LOG] 요청 시작
          console.groupCollapsed(
            `🌐 [Overpass Request] -> ${serverName} (Attempt: ${attempt})`,
          );
          console.log('%cQuery:', 'color: #007acc; font-weight: bold;', query);
          console.groupEnd();

          const response = await fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
            body: `data=${encodeURIComponent(query)}`,
            signal: controller.signal,
          });

          clearTimeout(timeoutId);

          if (response.status === 429) {
            throw new Error('Rate Limit Exceeded (429)');
          }

          if (!response.ok) {
            throw new Error(`HTTP Error: ${response.status}`);
          }

          const data = (await response.json()) as OverpassResponse;
          const endTime = performance.now();
          const duration = ((endTime - startTime) / 1000).toFixed(2);

          /**
           * 🚨 [핵심: 누락 여부 판단]
           * Overpass API는 런타임 에러나 데이터 제한 발생 시 'remark' 필드에 사유를 담아 보냅니다.
           * 이 경우 elements가 일부 포함되어 있더라도 '누락된 응답'으로 간주합니다.
           */
          if (data.osm3s?.remark) {
            const isRuntimeError = data.osm3s.remark.includes('runtime error');
            console.warn(
              `⚠️ [Partial Data] ${serverName} remark: ${data.osm3s.remark}`,
            );

            if (attempt < MAX_RETRY_PER_SERVER) {
              const delay = 1000 * attempt; // 점진적 딜레이
              console.log(`[Retry] ${delay}ms 후 동일 서버 재시도...`);
              await randomDelay(delay, delay + 500);
              continue; // 다음 attempt 실행
            }
            throw new Error(`Partial Response: ${data.osm3s.remark}`);
          }

          if (!data || !data.elements) {
            throw new Error('Invalid Response: Missing elements');
          }

          // [LOG] 성공 결과 출력
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
            error.name === 'AbortError' ? 'Timeout (15s)' : error.message;

          console.warn(
            `⚠️ [Overpass Failed] ${url} (${duration}s): ${errorMessage}`,
          );
          lastError = new Error(errorMessage);

          // HTTP 429나 심각한 서버 에러 시 즉시 다음 서버로 전환 (attempt 루프 탈출)
          break;
        }
      }
    }

    throw new Error(
      `[Overpass API] 모든 서버 응답 실패 또는 데이터 누락: ${lastError?.message}`,
    );
  });
}
