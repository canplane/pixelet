// @pxl/shared/spec/cell/utils.ts
// Bitmasking constants and utilities for cell processing.

// =============================================================================
// Bitmask Constants
// =============================================================================

/** Physical zero point every status check in this module is defined against. */
export const ZERO = 0x00;

/** Full 8-bit mask (all bits on). */
export const FULL = 0xff;

export const CELL_MASKS = {
  /** Sea flag (bit 7): 1 means a sea/deep-water cell (128). */
  SEA_FLAG: 0x80, // 1000 0000 (128)
  /** ID value mask (bits 0-6): extracts the pure material id (127). */
  ID_VALUE: 0x7f, // 0111 1111 (127)
} as const;
