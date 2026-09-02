// @pxl/shared/spec/cell/common.ts
// Shared state/constants across terrain / other cell layers.

export const ZERO = 0x00;
export const FULL = 0xff;

export const CELL_MASKS = {
  SEA_FLAG: 0x80, // 1000 0000 (128)
  ID_VALUE: 0x7f, // 0111 1111 (127)
} as const;

export const common_types = {
  /** valid empty space */
  AIR: ZERO,

  /** missing / invalid / unresolved data */
  UNKNOWN: FULL,
} as const;

export const common_colors = {
  [common_types.AIR]: null,
  [common_types.UNKNOWN]: "#D9D0CC",
} as const;
