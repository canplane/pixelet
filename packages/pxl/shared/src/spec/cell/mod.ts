// @pxl/shared/spec/cell/mod.ts
// Entry point for the cell model. Assembles and re-exports the modular definitions.

import { common_colors, common_types } from "./common.ts";
import { terrain_colors, terrain_types } from "./terrain.ts";

// =============================================================================
// Aggregated Types
// =============================================================================

export const cell_types = {
  /** Shared status values */
  common: common_types,

  /** Terrain layer: 2.5D canvas */
  terrain: terrain_types,
} as const;

// =============================================================================
// Aggregated Colors
// =============================================================================

export const cell_colors = {
  /** Shared status values */
  common: common_colors,

  /** Terrain layer: 2.5D canvas */
  terrain: terrain_colors,
} as const;
