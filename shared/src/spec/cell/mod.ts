// @pixelet/shared/spec/cell/mod.ts
// Cell 모델의 엔트리 포인트. 모듈화된 정의를 조립하여 내보냅니다.

import { common_colors, common_types } from "./common.ts";
import { terrain_colors, terrain_types } from "./terrain.ts";
// import { object_colors, object_types } from "./object.ts";

// =============================================================================
// Aggregated Types
// =============================================================================

export const cell_types = {
  /** 🌐 [COMMON] 공통 상태 값 */
  common: common_types,

  /** 📜 [Layer: terrain] 2.5D Canvas */
  terrain: terrain_types,
  // /** 🧊 [Layer: object] 3D Draft */
  // object: object_types,
} as const;

// =============================================================================
// Aggregated Colors
// =============================================================================

export const cell_colors = {
  /** 🌐 [COMMON] 공통 상태 값 */
  common: common_colors,

  /** 📜 [Layer: terrain] 2.5D Canvas */
  terrain: terrain_colors,
  // /** 🧊 [Layer: object] 3D Draft */
  // object: object_colors,
} as const;
