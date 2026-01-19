// // @pxl/common/lib/async-utils.ts

// /**
//  * 비동기 처리를 위한 공용 유틸리티 함수 모음
//  */

// /**
//  * 단순 지연 함수
//  */
// export const delay = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

// /**
//  * 랜덤 지연 함수 (Jitter)
//  * min ~ max 사이의 랜덤한 시간(ms)만큼 대기합니다.
//  * API 호출 시 기계적인 패턴을 피하거나 서버 부하를 분산하기 위해 사용됩니다.
//  */
// export const randomDelay = (min: number, max: number): Promise<void> => {
//   const ms = Math.floor(Math.random() * (max - min + 1) + min);
//   return new Promise((resolve) => setTimeout(resolve, ms));
// };

// /**
//  * 🎯 [Engineering Intuition]
//  * 네트워크 일시 오류나 레이트 리밋 상황에서
//  * 시스템의 가용성(Availability)을 극대화하기 위한 고성능 재시도 유틸입니다.
//  */
// export async function withRetry<T>(operation: () => Promise<T>, retries: number = 3, delay: number = 2000): Promise<T> {
//   try {
//     return await operation();
//   } catch (err) {
//     if (retries <= 0) throw err;
//     // 🎯 지수 백오프로 재시도 (점점 더 길게 대기)
//     await new Promise((resolve) => setTimeout(resolve, delay));
//     return withRetry(operation, retries - 1, delay * 2);
//   }
// }
