// packages/core/src/shared/lib/cache-utils.ts
import { generateHash } from './crypto-utils';

// 🎯 [Caching]: 이미 요청 중이거나 완료된 데이터를 기억하여 중복 API 호출을 물리적으로 차단합니다.
const globalPromiseCache = new Map<string, Promise<any>>();

/**
 * [withSharedPromise]
 * 동일한 키(해시 기반)를 가진 비동기 요청이 동시에 발생할 경우
 * 기존 프로미스를 공유하고, 성공 시 일정 시간 동안 결과를 유지하는 래퍼입니다.
 */
export async function withSharedPromise<T>(
  cachePrefix: string,
  rawKey: string,
  fetcher: () => Promise<T>,
  ttl: number = 30000,
): Promise<T> {
  // 1. 해시 생성
  const cacheKey = await generateHash(`${cachePrefix}:${rawKey}`);

  // 2. 해시값으로 캐시 체크
  if (globalPromiseCache.has(cacheKey)) {
    return globalPromiseCache.get(cacheKey)!;
  }

  // 3. 해시값으로 캐시 등록
  const task = fetcher();
  globalPromiseCache.set(cacheKey, task);

  // 4. 라이프사이클 관리 (성공 시 TTL 부여, 실패 시 즉시 삭제)
  task
    .then(() => {
      if (ttl > 0) {
        setTimeout(() => globalPromiseCache.delete(cacheKey), ttl);
      } else {
        globalPromiseCache.delete(cacheKey);
      }
    })
    .catch(() => {
      globalPromiseCache.delete(cacheKey);
    });

  return task;
}
