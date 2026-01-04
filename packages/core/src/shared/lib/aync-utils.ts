/**
 * @file async-utils.ts
 * 비동기 처리를 위한 공용 유틸리티 함수 모음
 */

/**
 * 단순 지연 함수
 */
export const delay = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

/**
 * 랜덤 지연 함수 (Jitter)
 * min ~ max 사이의 랜덤한 시간(ms)만큼 대기합니다.
 * API 호출 시 기계적인 패턴을 피하거나 서버 부하를 분산하기 위해 사용됩니다.
 */
export const randomDelay = (min: number, max: number): Promise<void> => {
  const ms = Math.floor(Math.random() * (max - min + 1) + min);
  return new Promise((resolve) => setTimeout(resolve, ms));
};
