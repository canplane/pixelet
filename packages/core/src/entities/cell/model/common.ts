/**
 * @file common.ts
 * [Layer: COMMON] 모든 레이어(BASE, BUILD)가 공유하는 공통 상태 규격
 */

import { ZERO, FULL } from './utils';

export const COMMON_TYPES = {
  /** 💨 유효한 빈 공간 (0x00): 모든 비트가 OFF인 상태 */
  AIR: ZERO,
  /** 🌫️ 데이터 부재/에러 (0xFF): 기준값(AIR)의 모든 비트를 반전시킨 ALL-ON 상태
   * 🎯 [Symmetry] ~(0x00) => 0xFF. 유틸리티는 이 물리적 역전 관계를 신뢰합니다.
   */
  UNKNOWN: FULL,
} as const;

export const COMMON_COLORS = {
  [COMMON_TYPES.AIR]: null,
  [COMMON_TYPES.UNKNOWN]: '#E0DDD5',
} as const;
