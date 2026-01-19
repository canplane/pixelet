// @pxl/spec/cell/common.ts
// [Layer: common] Shared state definitions across terrain / other cell layers

import { FULL, ZERO } from "./utils.ts";

// ============================================================================
// Shared Cell Types
// ============================================================================

export const common_types = {
  /** valid empty space */
  AIR: ZERO,

  /** missing / invalid / unresolved data */
  UNKNOWN: FULL,
} as const;

// ============================================================================
// Shared Cell Colors
// ============================================================================

export const common_colors = {
  [common_types.AIR]: null,
  [common_types.UNKNOWN]: "#D9D0CC",
} as const;
