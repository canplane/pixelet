// @pxl/web/modules/world/render/config.ts

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

/**
 * Render and camera tuning constants for the web world renderer.
 * Distances are expressed in current world units.
 */

/** Control knob for global view-dependent distances. */
const VIEW_DIST: f64 = 2048;

/** Default tilt angle in degrees from the ground plane. */
const DEFAULT_TILT_DEG: f64 = 30;

/** Camera projection and interaction limits. */
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

/** Render-distance and fog configuration. */
export const render_cfg = {
  FADE_START: VIEW_DIST * 0.75,
  FADE_END: VIEW_DIST,

  FAR_CLIP: VIEW_DIST * 1.5,

  FOG_NEAR: VIEW_DIST * 0.75,
  FOG_FAR: VIEW_DIST,
} as const;

/** Active owner of camera motion. */
export const enum CameraControlMode {
  NONE = "none",
  USER = "user",
  SYSTEM = "system",
}
