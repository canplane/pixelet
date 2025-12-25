/**
 * @file async-utils.ts
 * 비동기 처리를 위한 공용 유틸리티 함수 모음
 */

/**
 * 랜덤 지연 함수 (Jitter)
 * min ~ max 사이의 랜덤한 시간(ms)만큼 대기합니다.
 * API 호출 시 기계적인 패턴을 피하기 위해 사용됩니다.
 */
export const randomDelay = (min: number, max: number) => {
  const delay = Math.floor(Math.random() * (max - min + 1) + min);
  return new Promise((resolve) => setTimeout(resolve, delay));
};
