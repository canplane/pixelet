/**
 * @file build.ts
 * [Layer: BUILD] Chunk 단위로 로딩되는 3D 사용자 데이터
 */

import { COMMON_TYPES, COMMON_COLORS } from './common';

// -----------------------------------------------------------------------------
// ID Definitions
// -----------------------------------------------------------------------------

export const BUILD_TYPES = {
  ...COMMON_TYPES, // 0 is RESERVED for AIR/ERASER in the parent scope

  // --- [BASIC MATERIALS] ---

  /** ⬜ 기본 블록 (White) */
  SOLID: 1,
  /** 🧱 벽돌 */
  BRICK: 2,
  /** 🪵 나무 */
  WOOD: 3,
  /** 🪟 유리 */
  GLASS: 4,
  /** 🏢 콘크리트 */
  CONCRETE: 5,

  // --- [SPECIAL] ---

  /** 💡 조명 */
  LIGHT: 10,
} as const;

// -----------------------------------------------------------------------------
// Color Definitions
// -----------------------------------------------------------------------------

export const BUILD_COLORS = {
  ...COMMON_COLORS,

  // --- [BASIC MATERIALS] ---

  [BUILD_TYPES.SOLID]: '#FFFFFF',
  [BUILD_TYPES.BRICK]: '#C62828',
  [BUILD_TYPES.WOOD]: '#8D6E63',
  [BUILD_TYPES.GLASS]: '#90CAF9', // Alpha should be handled in shader/renderer
  [BUILD_TYPES.CONCRETE]: '#9E9E9E',

  // --- [SPECIAL] ---

  [BUILD_TYPES.LIGHT]: '#FFF176',
} as const;
