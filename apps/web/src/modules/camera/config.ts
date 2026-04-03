// @pxl/web/modules/camera/config.ts

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

/** Camera projection and interaction limits. */

/** Control knob for global view-dependent distances. */
export const VIEW_DIST: f64 = 2048;

/** Default tilt angle in degrees from the ground plane. */
export const DEFAULT_TILT_DEG: f64 = 30;

export const camera_cfg = {
  FOV: 50,
  NEAR: 0.1,

  DEFAULT_DIST: 128,

  /**
   * Polar angle in OrbitControls semantics.
   * 0 = straight down from above, PI / 2 = horizontal.
   */
  DEFAULT_POLAR_ANGLE: ((90 - DEFAULT_TILT_DEG) * Math.PI) / 180,

  MIN_DIST: 4,
  MAX_DIST: VIEW_DIST * 0.75,

  /** Snap instead of interpolating when the target moves farther than this. */
  TRAVEL_DIST_THRES: 250,
} as const;
