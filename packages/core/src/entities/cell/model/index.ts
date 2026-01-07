/**
 * @file index.ts
 * Cell 모델의 엔트리 포인트. 모듈화된 정의를 조립하여 내보냅니다.
 */

import { COMMON_TYPES, COMMON_COLORS } from './common';
import { BASE_TYPES, BASE_COLORS } from './base';
import { BUILD_TYPES, BUILD_COLORS } from './build';

// =============================================================================
// Aggregated Types
// =============================================================================

export const CELL_TYPES = {
  // /** 🌐 [COMMON] 공통 상태 값 */
  // COMMON: COMMON_TYPES,

  /** 📜 [Layer: BASE] 2.5D Canvas */
  BASE: BASE_TYPES,

  /** 🧊 [Layer: BUILD] 3D Draft */
  BUILD: BUILD_TYPES,
} as const;

// =============================================================================
// Aggregated Colors
// =============================================================================

export const CELL_COLORS = {
  // /** 🌐 [COMMON] 공통 상태 값 */
  // COMMON: COMMON_COLORS,

  /** 📜 [Layer: BASE] 2.5D Canvas */
  BASE: BASE_COLORS,

  /** 🧊 [Layer: BUILD] 3D Draft */
  BUILD: BUILD_COLORS,
} as const;
